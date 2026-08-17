#![allow(missing_docs)]
use glyim_codegen::CodegenBackend;
use glyim_codegen_llvm::LlvmBackend;
use glyim_db::Database;
use glyim_diag::{CompResult, DiagSink, GlyimDiagnostic};
use glyim_lower::mono::MonoCtx;
use glyim_lower::post_mono_checks::{
    check_large_mono_set, check_unsized_locals, check_unused_generic_params,
};
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

/// Artifacts produced by compiling a source file through the full pipeline.
/// Returned by [`Pipeline::compile_file_with_artifacts`] so that test harnesses
/// can inspect the mid-pipeline results (def-map, type-check, MIR bodies) that
/// the standard `compile_file` discards.
pub struct CompileArtifacts {
    pub def_map: glyim_def_map::CrateDefMap,
    pub typeck_result: glyim_typeck::TypeckResult,
    pub mir_bodies: Vec<Arc<glyim_mir::Body>>,
    pub ty_ctx: Arc<glyim_type::TyCtx>,
}

impl Pipeline {
    /// Production entry point: compile a source file to an object file and
    /// discard the intermediate artifacts. Kept for `glyip` and other callers
    /// that only need the object file; test harnesses should use
    /// [`Pipeline::compile_file_with_artifacts`] to inspect the def-map /
    /// type-check / MIR results.
    pub fn compile_file(
        db: &mut Database,
        path: &Path,
        backend: &dyn CodegenBackend,
        output_path: &Path,
    ) -> CompResult<()> {
        Self::compile_file_with_artifacts(db, path, backend, output_path).map(|_| ())
    }
}

impl Pipeline {
    /// Compile a source file through the full pipeline and return the mid-pipeline
    /// artifacts (def-map, type-check result, MIR bodies, type context) in addition
    /// to emitting the object file. Used by test harnesses that need to assert on
    /// the MIR/typeck output.
    pub fn compile_file_with_artifacts(
        db: &mut Database,
        path: &Path,
        backend: &dyn CodegenBackend,
        output_path: &Path,
    ) -> CompResult<CompileArtifacts> {
        let sink = DiagSink::new();
        let sink_cell = RefCell::new(sink);

        let file_id = db
            .vfs()
            .add_file_from_disk(path)
            .map_err(|e| vec![GlyimDiagnostic::internal_error(format!("I/O Error: {}", e))])?;
        let source = db
            .vfs()
            .file_content(file_id)
            .unwrap_or_else(|| Arc::from(""));

        let parse_result = glyim_frontend::parse_to_syntax(&source, file_id);
        sink_cell
            .borrow_mut()
            .extend(parse_result.diagnostics.clone());
        if sink_cell.borrow().has_errors() {
            return Err(sink_cell.into_inner().into_diagnostics());
        }

        let (def_map, def_diagnostics) =
            glyim_def_map::build_def_map(&parse_result.root, db.krate());
        sink_cell.borrow_mut().extend(def_diagnostics);
        if sink_cell.borrow().has_errors() {
            return Err(sink_cell.into_inner().into_diagnostics());
        }

        let (hir, hir_diags) =
            glyim_hir::pipeline_api::lower_crate_for_pipeline(&parse_result.root, db.intern_mut());
        sink_cell.borrow_mut().extend(hir_diags);

        let resolver = db.interner().clone();
        let ty_ctx_mut = glyim_type::TyCtxMut::new(resolver);
        let trait_ctx = glyim_solve::TraitContext::new();
        let mut solver = SimpleTraitSolver::new(&trait_ctx);
        let (ty_ctx, typeck_result) =
            glyim_typeck::typeck_crate(ty_ctx_mut, &def_map, &hir, &mut solver);
        sink_cell.borrow_mut().extend(typeck_result.diagnostics.clone());
        if sink_cell.borrow().has_errors() {
            return Err(sink_cell.into_inner().into_diagnostics());
        }

        db.set_ty_ctx(ty_ctx);

        let mir_bodies_map: std::collections::HashMap<glyim_core::def_id::DefId, Arc<Body>> = {
            let ty_ctx_guard = db.get_ty_ctx().expect("TyCtx not initialized");
            let ty_ctx_ref = ty_ctx_guard.as_ref();

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
            let ty_ctx_guard = db.get_ty_ctx().expect("TyCtx not initialized");
            let ty_ctx_ref = ty_ctx_guard.as_ref();
            let body_provider = make_mir_body_provider(&mir_bodies_map, &sink_cell, ty_ctx_ref);
            let drop_provider = make_drop_glue_provider(ty_ctx_ref);
            mono_ctx.collect(&mono_roots, &body_provider, &drop_provider);
            // §8.12: post-monomorphization semantic checks. These were dead
            // `#[allow(dead_code)]` functions; they now run here, immediately
            // after monomorphization, so unsized locals / unused generic params
            // / pathological mono-set sizes surface as diagnostics instead of
            // being silently ignored.
            let mut post_diags = check_unsized_locals(mono_ctx.items(), ty_ctx_ref);
            post_diags.extend(check_unused_generic_params(
                mono_ctx.items(),
                ty_ctx_ref,
            ));
            post_diags.extend(check_large_mono_set(mono_ctx.items(), 1000));
            sink_cell.borrow_mut().extend(post_diags);
            mono_ctx.items().to_vec()
        };

        if sink_cell.borrow().has_errors() {
            return Err(sink_cell.into_inner().into_diagnostics());
        }

        let cache = PipelineMonoCache::from_items(&mono_items);
        db.set_mono_cache(cache.symbols().to_vec());

        let max_cgus = compute_max_cgus();
        let cgus = partition(&mono_items, max_cgus);

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

        let ty_ctx = db.get_ty_ctx().expect("TyCtx not initialized");
        Ok(CompileArtifacts {
            def_map,
            typeck_result: typeck_result.clone(),
            mir_bodies: all_bodies,
            ty_ctx,
        })
    }
}

/// Result of compiling a source file down to MIR without code generation.
pub struct MirCompilation {
    /// All monomorphized MIR bodies, keyed by their owning `DefId`.
    pub bodies: std::collections::HashMap<glyim_core::def_id::DefId, Arc<Body>>,
    /// The crate's definition map (used to resolve a function name to a `DefId`).
    pub def_map: glyim_def_map::CrateDefMap,
    /// The type context produced during type-checking (required by the interpreter).
    pub ty_ctx: Arc<glyim_type::TyCtx>,
}

/// Compile a single source file to MIR only (no backend code generation).
///
/// This is the entry point used by `glyip test` and other runners that execute
/// MIR bodies directly (e.g. via the in-process `glyim_mir_interp` interpreter)
/// instead of producing a native object file.
pub fn compile_file_to_mir(
    db: &mut Database,
    path: &Path,
) -> Result<MirCompilation, Vec<GlyimDiagnostic>> {
    let sink = DiagSink::new();
    let sink_cell = RefCell::new(sink);

    let file_id = db
        .vfs()
        .add_file_from_disk(path)
        .map_err(|e| vec![GlyimDiagnostic::internal_error(format!("I/O Error: {}", e))])?;
    let source = db
        .vfs()
        .file_content(file_id)
        .unwrap_or_else(|| Arc::from(""));

    let parse_result = glyim_frontend::parse_to_syntax(&source, file_id);
    sink_cell
        .borrow_mut()
        .extend(parse_result.diagnostics.clone());
    if sink_cell.borrow().has_errors() {
        return Err(sink_cell.into_inner().into_diagnostics());
    }

    let (def_map, def_diagnostics) = glyim_def_map::build_def_map(&parse_result.root, db.krate());
    sink_cell.borrow_mut().extend(def_diagnostics);
    if sink_cell.borrow().has_errors() {
        return Err(sink_cell.into_inner().into_diagnostics());
    }

    let (hir, hir_diags) =
        glyim_hir::pipeline_api::lower_crate_for_pipeline(&parse_result.root, db.intern_mut());
    sink_cell.borrow_mut().extend(hir_diags);

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

    let ty_ctx_guard = db.get_ty_ctx().expect("TyCtx not initialized");
    let ty_ctx_ref = ty_ctx_guard.as_ref();
    let lower_ctx = PipelineLowerCtx::new(ty_ctx_ref, &hir);

    let mut bodies = std::collections::HashMap::new();
    for (_owner_def_id, thir_body) in &typeck_result.thir_bodies {
        let lower_result = glyim_lower::lower_body(&lower_ctx, thir_body);
        sink_cell.borrow_mut().extend(lower_result.diagnostics);
        if sink_cell.borrow().has_errors() {
            return Err(sink_cell.into_inner().into_diagnostics());
        }
        let mir_arc = Arc::new(lower_result.body);
        let owner = mir_arc.owner;
        bodies.insert(owner, mir_arc);
    }

    Ok(MirCompilation {
        bodies,
        def_map,
        ty_ctx: ty_ctx_guard,
    })
}

pub fn emit_mir(
    db: &mut Database,
    input: &Path,
    output: &Path,
) -> Result<(), Vec<GlyimDiagnostic>> {
    let sink = DiagSink::new();
    let sink_cell = RefCell::new(sink);

    let file_id = db
        .vfs()
        .add_file_from_disk(input)
        .map_err(|e| vec![GlyimDiagnostic::internal_error(format!("I/O Error: {}", e))])?;
    let source = db
        .vfs()
        .file_content(file_id)
        .unwrap_or_else(|| Arc::from(""));

    let parse_result = glyim_frontend::parse_to_syntax(&source, file_id);
    sink_cell
        .borrow_mut()
        .extend(parse_result.diagnostics.clone());
    if sink_cell.borrow().has_errors() {
        return Err(sink_cell.into_inner().into_diagnostics());
    }

    let (def_map, def_diagnostics) = glyim_def_map::build_def_map(&parse_result.root, db.krate());
    sink_cell.borrow_mut().extend(def_diagnostics);
    if sink_cell.borrow().has_errors() {
        return Err(sink_cell.into_inner().into_diagnostics());
    }

    let (hir, hir_diags) =
        glyim_hir::pipeline_api::lower_crate_for_pipeline(&parse_result.root, db.intern_mut());
    sink_cell.borrow_mut().extend(hir_diags);

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

    let ty_ctx_guard = db.get_ty_ctx().expect("TyCtx not initialized");
    let ty_ctx_ref = ty_ctx_guard.as_ref();
    let lower_ctx = PipelineLowerCtx::new(ty_ctx_ref, &hir);
    let mut mir_bodies = Vec::new();

    for (_owner_def_id, thir_body) in &typeck_result.thir_bodies {
        let lower_result = glyim_lower::lower_body(&lower_ctx, thir_body);
        sink_cell.borrow_mut().extend(lower_result.diagnostics);
        if sink_cell.borrow().has_errors() {
            return Err(sink_cell.into_inner().into_diagnostics());
        }
        mir_bodies.push(lower_result.body);
    }

    let mut out = String::new();
    for body in &mir_bodies {
        out.push_str(&format_body(body, ty_ctx_ref));
        out.push_str("\n\n");
    }
    std::fs::write(output, out)
        .map_err(|e| vec![GlyimDiagnostic::internal_error(e.to_string())])?;

    Ok(())
}

pub fn emit_llvm_ir(
    db: &mut Database,
    input: &Path,
    output: &Path,
) -> Result<(), Vec<GlyimDiagnostic>> {
    let sink = DiagSink::new();
    let sink_cell = RefCell::new(sink);

    let file_id = db
        .vfs()
        .add_file_from_disk(input)
        .map_err(|e| vec![GlyimDiagnostic::internal_error(format!("I/O Error: {}", e))])?;
    let source = db
        .vfs()
        .file_content(file_id)
        .unwrap_or_else(|| Arc::from(""));

    let parse_result = glyim_frontend::parse_to_syntax(&source, file_id);
    sink_cell
        .borrow_mut()
        .extend(parse_result.diagnostics.clone());
    if sink_cell.borrow().has_errors() {
        return Err(sink_cell.into_inner().into_diagnostics());
    }

    let (def_map, def_diagnostics) = glyim_def_map::build_def_map(&parse_result.root, db.krate());
    sink_cell.borrow_mut().extend(def_diagnostics);
    if sink_cell.borrow().has_errors() {
        return Err(sink_cell.into_inner().into_diagnostics());
    }

    let (hir, hir_diags) =
        glyim_hir::pipeline_api::lower_crate_for_pipeline(&parse_result.root, db.intern_mut());
    sink_cell.borrow_mut().extend(hir_diags);

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

    let ty_ctx_guard = db.get_ty_ctx().expect("TyCtx not initialized");
    let ty_ctx_ref = ty_ctx_guard.as_ref();
    let lower_ctx = PipelineLowerCtx::new(ty_ctx_ref, &hir);
    let mut mir_bodies = Vec::new();

    for (_owner_def_id, thir_body) in &typeck_result.thir_bodies {
        let lower_result = glyim_lower::lower_body(&lower_ctx, thir_body);
        sink_cell.borrow_mut().extend(lower_result.diagnostics);
        if sink_cell.borrow().has_errors() {
            return Err(sink_cell.into_inner().into_diagnostics());
        }
        mir_bodies.push(lower_result.body);
    }

    if mir_bodies.is_empty() {
        return Err(vec![GlyimDiagnostic::internal_error(
            "No MIR bodies generated",
        )]);
    }

    let backend = LlvmBackend::new().with_debug_info(false);
    let ir = backend
        .emit_ir_to_string(ty_ctx_ref, &mir_bodies[0])
        .map_err(|e| {
            vec![GlyimDiagnostic::internal_error(format!(
                "LLVM IR generation failed: {:?}",
                e
            ))]
        })?;
    std::fs::write(output, ir).map_err(|e| vec![GlyimDiagnostic::internal_error(e.to_string())])?;

    Ok(())
}

/// Compile a single `.g` file to **assembly** (`.s`) and write it to `output`.
/// Mirrors [`emit_llvm_ir`] but lowers through the LLVM backend's
/// `emit_assembly` path (plan §18.3: `--emit=asm`).
pub fn emit_asm(
    db: &mut Database,
    input: &Path,
    output: &Path,
) -> Result<(), Vec<GlyimDiagnostic>> {
    let sink = DiagSink::new();
    let sink_cell = RefCell::new(sink);

    let file_id = db
        .vfs()
        .add_file_from_disk(input)
        .map_err(|e| vec![GlyimDiagnostic::internal_error(format!("I/O Error: {}", e))])?;
    let source = db
        .vfs()
        .file_content(file_id)
        .unwrap_or_else(|| Arc::from(""));

    let parse_result = glyim_frontend::parse_to_syntax(&source, file_id);
    sink_cell
        .borrow_mut()
        .extend(parse_result.diagnostics.clone());
    if sink_cell.borrow().has_errors() {
        return Err(sink_cell.into_inner().into_diagnostics());
    }

    let (def_map, def_diagnostics) = glyim_def_map::build_def_map(&parse_result.root, db.krate());
    sink_cell.borrow_mut().extend(def_diagnostics);
    if sink_cell.borrow().has_errors() {
        return Err(sink_cell.into_inner().into_diagnostics());
    }

    let (hir, hir_diags) =
        glyim_hir::pipeline_api::lower_crate_for_pipeline(&parse_result.root, db.intern_mut());
    sink_cell.borrow_mut().extend(hir_diags);

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

    let ty_ctx_guard = db.get_ty_ctx().expect("TyCtx not initialized");
    let ty_ctx_ref = ty_ctx_guard.as_ref();
    let lower_ctx = PipelineLowerCtx::new(ty_ctx_ref, &hir);
    let mut mir_bodies = Vec::new();

    for (_owner_def_id, thir_body) in &typeck_result.thir_bodies {
        let lower_result = glyim_lower::lower_body(&lower_ctx, thir_body);
        sink_cell.borrow_mut().extend(lower_result.diagnostics);
        if sink_cell.borrow().has_errors() {
            return Err(sink_cell.into_inner().into_diagnostics());
        }
        mir_bodies.push(lower_result.body);
    }

    if mir_bodies.is_empty() {
        return Err(vec![GlyimDiagnostic::internal_error(
            "No MIR bodies generated",
        )]);
    }

    let backend = LlvmBackend::new().with_debug_info(false);
    let arc_bodies: Vec<Arc<Body>> = mir_bodies.into_iter().map(Arc::new).collect();
    backend
        .emit_assembly(&arc_bodies, output)
        .map_err(|e| vec![GlyimDiagnostic::internal_error(format!("LLVM assembly generation failed: {:?}", e))])?;

    Ok(())
}

fn format_body(body: &Body, ctx: &glyim_type::TyCtx) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    writeln!(s, "fn {}() {{", body.owner).unwrap();
    for (idx, local) in body.locals.iter_enumerated() {
        writeln!(
            s,
            "  ${}: {}",
            idx.to_raw(),
            glyim_type::PrintTy::new(local.ty, ctx)
        )
        .unwrap();
    }
    for (idx, block) in body.basic_blocks.iter_enumerated() {
        writeln!(s, "bb{}:", idx.to_raw()).unwrap();
        for stmt in &block.statements {
            writeln!(s, "  {:?}", stmt.kind).unwrap();
        }
        writeln!(s, "  {:?}", block.terminator.kind).unwrap();
    }
    writeln!(s, "}}").unwrap();
    s
}

#[cfg(test)]
mod tests;
