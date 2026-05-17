use glyim_codegen::CodegenBackend;
use glyim_db::Database;
use glyim_diag::{CompResult, DiagSink, GlyimDiagnostic};
use glyim_lower::mono::MonoCtx;
use glyim_lower::partition::partition;
use glyim_mir::Body;
use glyim_solve::SimpleTraitSolver;
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;

mod mono_cache;
mod pipeline_context;
use mono_cache::{
    PipelineMonoCache, compute_max_cgus, make_drop_glue_provider, make_mir_body_provider,
};
use pipeline_context::{PipelineBorrowckCtx, PipelineLowerCtx};

pub struct Pipeline;

impl Pipeline {
    pub fn compile_file(
        db: &mut Database,
        path: &Path,
        backend: &dyn CodegenBackend,
        output_path: &Path,
    ) -> CompResult<()> {
        let mut sink = DiagSink::new();

        // Phase 1: VFS
        let file_id = db
            .vfs()
            .add_file_from_disk(path)
            .map_err(|e| vec![GlyimDiagnostic::internal_error(format!("I/O Error: {}", e))])?;
        let source = db
            .vfs()
            .file_content(file_id)
            .unwrap_or_else(|| Arc::from(""));

        // Phase 2: Parse
        let parse_result = glyim_frontend::parse_to_syntax(&source, file_id);
        sink.extend(parse_result.diagnostics.clone());
        if sink.has_errors() {
            return Err(sink.into_diagnostics());
        }

        // Phase 3: DefMap
        let (def_map, def_diagnostics) =
            glyim_def_map::build_def_map(&parse_result.root, db.krate());
        sink.extend(def_diagnostics);
        if sink.has_errors() {
            return Err(sink.into_diagnostics());
        }

        // Phase 4: HIR
        let hir =
            glyim_hir::pipeline_api::lower_crate_for_pipeline(&parse_result.root, db.intern_mut());

        // Phase 5: Typeck
        let resolver = db.interner().clone();
        let ty_ctx_mut = glyim_type::TyCtxMut::new(resolver);
        let trait_ctx = glyim_solve::TraitContext::new();
        let mut solver = SimpleTraitSolver::new(&trait_ctx);
        let (ty_ctx, typeck_result) =
            glyim_typeck::typeck_crate(ty_ctx_mut, &def_map, &hir, &mut solver);
        sink.extend(typeck_result.diagnostics);
        if sink.has_errors() {
            return Err(sink.into_diagnostics());
        }

        db.set_ty_ctx(ty_ctx);

        // Phase 6: MIR lowering, borrow checking, optimization
        // Build a map from DefId → optimized MIR body for later lookup by
        // the monomorphization collector.
        let mir_bodies_map: std::collections::HashMap<glyim_core::def_id::DefId, Arc<Body>> = {
            let ty_ctx_guard = db.ty_ctx();
            let ty_ctx = ty_ctx_guard.as_ref().expect("ty_ctx not set after typeck");

            let lower_ctx = PipelineLowerCtx::new(ty_ctx, &hir);
            let mut bodies = std::collections::HashMap::new();

            for (_owner_def_id, thir_body) in &typeck_result.thir_bodies {
                let lower_result = glyim_lower::lower_body(&lower_ctx, thir_body);
                sink.extend(lower_result.diagnostics);
                if sink.has_errors() {
                    return Err(sink.into_diagnostics());
                }
                let mir_body = lower_result.body;
                let owner = mir_body.owner;
                let mir_arc = Arc::new(mir_body);

                let borrowck_ctx = PipelineBorrowckCtx::new(ty_ctx, &mir_arc);
                let borrowck_result = glyim_borrowck::check_borrows(&borrowck_ctx, &mir_arc);
                sink.extend(borrowck_result.errors);
                if sink.has_errors() {
                    return Err(sink.into_diagnostics());
                }

                let opt_body = glyim_opt::optimize(ty_ctx, &mir_arc);
                bodies.insert(owner, Arc::new(opt_body.body));
            }
            bodies
        }; // ty_ctx_guard dropped here

        // Phase 7: Monomorphization — discover roots and collect items
        let (mono_roots, discovery_diags) = {
            // discover_mono_roots needs TyCtxMut for intern_substitution.
            // Create a fresh one from the interner (which is shared).
            let mut ty_ctx_mut_for_discovery = glyim_type::TyCtxMut::new(db.interner().clone());
            glyim_lower::discovery::discover_mono_roots(
                &parse_result.root,
                &hir,
                &mut ty_ctx_mut_for_discovery,
            )
        };
        sink.extend(discovery_diags);

        // Collect mono items using MonoCtx with the pre-lowered MIR bodies.
        // The body_provider borrows mir_bodies_map, so we scope the collect
        // call so the borrow is released before we consume mir_bodies_map.
        let mono_items: Vec<glyim_lower::mono::MonoItemData> = {
            let mut mono_ctx = MonoCtx::new();
            let body_provider = make_mir_body_provider(&mir_bodies_map);
            let drop_provider = make_drop_glue_provider();
            mono_ctx.collect(&mono_roots, &body_provider, &drop_provider);
            mono_ctx.items().to_vec()
        }; // body_provider borrow released here

        // Update the Database mono cache for potential reuse
        let cache = PipelineMonoCache::from_items(&mono_items);
        db.set_mono_cache(cache.symbols().to_vec());

        // Phase 8: Partition into codegen units (CGUs)
        let max_cgus = compute_max_cgus();
        let cgus = partition(&mono_items, max_cgus);

        // Phase 9: Parallel codegen per CGU
        // Collect bodies from all CGUs. For v0.1.0 we aggregate and call
        // backend.generate() once, but the CGU structure enables future
        // parallel object file emission.
        let all_bodies: Vec<Arc<Body>> = if cgus.is_empty() {
            // No mono items discovered (e.g., no main function) — fall back
            // to passing all optimized MIR bodies directly.
            mir_bodies_map.into_values().collect()
        } else {
            // Use rayon to pre-process CGUs in parallel (validation,
            // statistics), then flatten into a single body list.
            let _cgu_stats: Vec<(usize, usize)> = cgus
                .par_iter()
                .map(|cgu_indices| {
                    let body_count = cgu_indices.len();
                    let total_locals: usize = cgu_indices
                        .iter()
                        .map(|&idx| mono_items[idx].body.locals.len())
                        .sum();
                    (body_count, total_locals)
                })
                .collect();

            // Flatten CGU bodies in deterministic order
            cgus.iter()
                .flat_map(|cgu_indices| cgu_indices.iter().map(|&idx| mono_items[idx].body.clone()))
                .collect()
        };

        // Phase 10: Invoke the codegen backend
        let out_path = if output_path.as_os_str().is_empty() {
            Path::new("output.o")
        } else {
            output_path
        };

        if !all_bodies.is_empty() {
            backend.generate(&all_bodies, out_path)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
