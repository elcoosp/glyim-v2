use glyim_codegen::CodegenBackend;
#[cfg(test)]
mod tests;
use glyim_db::Database;
use glyim_diag::{CompResult, DiagSink, GlyimDiagnostic};
use glyim_mir::Body;
use glyim_solve::SimpleTraitSolver;
use std::path::Path;
use std::sync::Arc;

mod pipeline_context;
use pipeline_context::{PipelineBorrowckCtx, PipelineLowerCtx};

pub struct Pipeline;

impl Pipeline {
    pub fn compile_file(
        db: &mut Database,
        path: &Path,
        backend: &dyn CodegenBackend,
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
        let hir = glyim_hir::pipeline_api::lower_crate_for_pipeline(
            &parse_result.root,
            db.intern_mut(),
        );

        // Phase 5: Typeck
        let resolver = db.interner().clone();
        let ty_ctx_mut = glyim_type::TyCtxMut::new(resolver);
        let mut solver = SimpleTraitSolver::new(db.trait_ctx());
        let (ty_ctx, typeck_result) =
            glyim_typeck::typeck_crate(ty_ctx_mut, &def_map, &hir, &mut solver);
        sink.extend(typeck_result.diagnostics);
        if sink.has_errors() {
            return Err(sink.into_diagnostics());
        }

        db.set_ty_ctx(ty_ctx);

        // Phase 6: MIR lowering, borrow checking, optimization
        // We need TyCtx data but must not hold the RwLock guard across phases.
        // Clone the necessary data and drop the guard.
        let optimized_bodies: Vec<Arc<Body>> = {
            let ty_ctx_guard = db.ty_ctx();
            let ty_ctx = ty_ctx_guard.as_ref().expect("ty_ctx not set after typeck");

            let lower_ctx = PipelineLowerCtx::new(ty_ctx, &hir);
            let mut bodies = Vec::new();

            for (_owner_def_id, thir_body) in &typeck_result.thir_bodies {
                let lower_result = glyim_lower::lower_body(&lower_ctx, thir_body);
                sink.extend(lower_result.diagnostics);
                if sink.has_errors() {
                    return Err(sink.into_diagnostics());
                }
                let mir_body = lower_result.body;
                let mir_arc = Arc::new(mir_body);

                let borrowck_ctx = PipelineBorrowckCtx::new(ty_ctx, &mir_arc);
                let borrowck_result = glyim_borrowck::check_borrows(&borrowck_ctx, &mir_arc);
                sink.extend(borrowck_result.errors);
                if sink.has_errors() {
                    return Err(sink.into_diagnostics());
                }

                let opt_body = glyim_opt::optimize(ty_ctx, &mir_arc);
                bodies.push(Arc::new(opt_body.body));
            }
            bodies
        }; // ty_ctx_guard dropped here

        // Phase 7: Codegen
        backend.generate(&optimized_bodies, Path::new("output.o"))?;

        Ok(())
    }
}
