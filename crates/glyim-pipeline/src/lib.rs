use glyim_codegen::CodegenBackend;
use glyim_db::Database;
use glyim_diag::{CompResult, DiagSink, GlyimDiagnostic};
use glyim_mir::Body;
use glyim_solve::SimpleTraitSolver;
use std::path::Path;
use std::sync::Arc;

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

        // Phase 4: HIR (stub)
        let hir = glyim_hir::CrateHir {
            items: glyim_core::arena::IndexVec::new(),
            bodies: glyim_core::arena::IndexVec::new(),
            body_owners: glyim_core::arena::IndexVec::new(),
        };

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

        // Phase 6-7: MIR and optimizations (stub: create a dummy MIR body)
        let dummy_owner = glyim_core::def_id::DefId::new(
            glyim_core::def_id::CrateId::from_raw(0),
            glyim_core::def_id::LocalDefId::from_raw(0),
        );
        let dummy_body = Arc::new(Body::dummy(dummy_owner));
        let optimized_bodies = vec![dummy_body];

        // Phase 8: Codegen
        backend.generate(&optimized_bodies, Path::new("output.o"))?;

        Ok(())
    }
}
