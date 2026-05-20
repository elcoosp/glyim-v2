use glyim_codegen::CodegenBackend;
use glyim_db::Database;
use glyim_diag::{CompResult, DiagSink, GlyimDiagnostic};
use glyim_lower::mono::MonoCtx;
use glyim_lower::partition::partition;
use glyim_mir::Body;
use glyim_solve::SimpleTraitSolver;
use rayon::prelude::*;
use std::cell::RefCell;
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
        let sink = DiagSink::new();
        let sink_cell = RefCell::new(sink);

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
        sink_cell
            .borrow_mut()
            .extend(parse_result.diagnostics.clone());
        if sink_cell.borrow().has_errors() {
            return Err(sink_cell.into_inner().into_diagnostics());
        }

        // Phase 3: DefMap
        let (def_map, def_diagnostics) =
            glyim_def_map::build_def_map(&parse_result.root, db.krate());
        sink_cell.borrow_mut().extend(def_diagnostics);
        if sink_cell.borrow().has_errors() {
            return Err(sink_cell.into_inner().into_diagnostics());
        }

        // Phase 4: HIR
        let (hir, hir_diags) =
            glyim_hir::pipeline_api::lower_crate_for_pipeline(&parse_result.root, db.intern_mut());
        sink_cell.borrow_mut().extend(hir_diags);

        // Phase 5: Typeck
        let resolver = db.interner().clone();
        let ty_ctx_mut = glyim_type::TyCtxMut::new(resolver);
        let trait_ctx = glyim_solve::TraitContext::new();
        let mut solver = SimpleTraitSolver::new(&trait_ctx);
        let (ty_ctx, typeck_result) =
            glyim_typeck::typeck_crate(ty_ctx_mut, &def_map, &hir, &mut solver);
        sink_cell.borrow_mut().extend(typeck_result.diagnostics);
        if sink_cell.borrow().has_errors() {
            return Err(sink_cell.into_inner().into_diagnostics());
        }

        db.set_ty_ctx(ty_ctx);

        // Phase 6: MIR lowering, borrow checking, optimization
        let mir_bodies_map: std::collections::HashMap<glyim_core::def_id::DefId, Arc<Body>> = {
            let ty_ctx_guard = db.ty_ctx();
            let ty_ctx_ref = ty_ctx_guard.as_ref().expect("ty_ctx not set after typeck");

            let lower_ctx = PipelineLowerCtx::new(ty_ctx_ref, &hir);
            let mut bodies = std::collections::HashMap::new();

            for (_owner_def_id, thir_body) in &typeck_result.thir_bodies {
                let lower_result = glyim_lower::lower_body(&lower_ctx, thir_body);
                sink_cell.borrow_mut().extend(lower_result.diagnostics);
                if sink_cell.borrow().has_errors() {
                    return Err(sink_cell.into_inner().into_diagnostics());
                }
                let mir_body = lower_result.body;
                let owner = mir_body.owner;
                let mir_arc = Arc::new(mir_body);

                let borrowck_ctx = PipelineBorrowckCtx::new(ty_ctx_ref, &mir_arc);
                let borrowck_result = glyim_borrowck::check_borrows(&borrowck_ctx, &mir_arc);
                sink_cell.borrow_mut().extend(borrowck_result.errors);
                if sink_cell.borrow().has_errors() {
                    return Err(sink_cell.into_inner().into_diagnostics());
                }

                let opt_body = glyim_opt::optimize(ty_ctx_ref, &mir_arc);
                bodies.insert(owner, Arc::new(opt_body.body));
            }
            bodies
        };

        // Phase 7: Monomorphization — discover roots and collect items
        let (mono_roots, discovery_diags) = {
            let mut ty_ctx_mut_for_discovery = glyim_type::TyCtxMut::new(db.interner().clone());
            glyim_lower::discovery::discover_mono_roots(
                &parse_result.root,
                &hir,
                &mut ty_ctx_mut_for_discovery,
            )
        };
        sink_cell.borrow_mut().extend(discovery_diags);

        let mono_items: Vec<glyim_lower::mono::MonoItemData> = {
            let mut mono_ctx = MonoCtx::new();
            let ty_ctx_guard = db.ty_ctx();
            let ty_ctx_ref = ty_ctx_guard.as_ref().expect("ty_ctx not set");
            let body_provider = make_mir_body_provider(&mir_bodies_map, &sink_cell, ty_ctx_ref);
            let drop_provider = make_drop_glue_provider(ty_ctx_ref);
            mono_ctx.collect(&mono_roots, &body_provider, &drop_provider);
            mono_ctx.items().to_vec()
        };

        // Check for errors emitted during body provider lookups
        if sink_cell.borrow().has_errors() {
            return Err(sink_cell.into_inner().into_diagnostics());
        }

        let cache = PipelineMonoCache::from_items(&mono_items);
        db.set_mono_cache(cache.symbols().to_vec());

        // Phase 8: Partition into codegen units (CGUs)
        let max_cgus = compute_max_cgus();
        let cgus = partition(&mono_items, max_cgus);

        // Phase 9: Parallel codegen per CGU
        let all_bodies: Vec<Arc<Body>> = if cgus.is_empty() {
            mir_bodies_map.into_values().collect()
        } else {
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

            cgus.iter()
                .flat_map(|cgu_indices| cgu_indices.iter().map(|&idx| mono_items[idx].body.clone()))
                .collect()
        };

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
