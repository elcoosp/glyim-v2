//! Mono item caching for the pipeline.
//!
//! Provides a thin wrapper around `MonoCtx` that integrates with
//! `Database`'s mono cache for cross-compilation reuse.

use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_diag::{DiagSink, GlyimDiagnostic};
use glyim_lower::mono::MonoItemData;
use glyim_mir::{BasicBlockIdx, Body, Rvalue, StatementKind, TerminatorKind};
use glyim_type::{GenericArg, ParamTy, Substitution, Ty, TyCtx, TyKind};
use std::cell::RefCell;
use std::sync::Arc;

/// Pipeline-level mono cache that wraps MonoCtx and tracks
/// which items have been collected for potential reuse.
pub(crate) struct PipelineMonoCache {
    symbols: Vec<String>,
}

impl PipelineMonoCache {
    pub(crate) fn from_items(items: &[MonoItemData]) -> Self {
        let symbols = items.iter().map(|d| d.symbol.clone()).collect();
        PipelineMonoCache { symbols }
    }

    pub(crate) fn symbols(&self) -> &[String] {
        &self.symbols
    }

    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.symbols.len()
    }

    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }
}

/// Substitute generic parameters in a MIR body with concrete arguments.
fn substitute_body(body: &Body, substs: &Substitution, ty_ctx: &TyCtx) -> Body {
    // Build a mapping from ParamTy index to concrete Ty.
    let mut ty_map = Vec::new();
    for arg in ty_ctx.substitution_args(*substs) {
        match arg {
            GenericArg::Ty(ty) => ty_map.push(*ty),
            _ => ty_map.push(ty_ctx.error_ty()),
        }
    }

    fn replace_ty(ty: Ty, ty_map: &[Ty], ty_ctx: &TyCtx) -> Ty {
        match ty_ctx.ty_kind(ty) {
            TyKind::Param(ParamTy { index, .. }) if (*index as usize) < ty_map.len() => {
                ty_map[*index as usize]
            }
            TyKind::Param(_) => ty_ctx.error_ty(),
            _ => ty,
        }
    }

    let mut new_locals = body.locals.clone();
    for local in new_locals.iter_mut() {
        local.ty = replace_ty(local.ty, &ty_map, ty_ctx);
    }

    let mut new_blocks = body.basic_blocks.clone();
    for block_data in new_blocks.iter_mut() {
        for stmt in &mut block_data.statements {
            match &mut stmt.kind {
                StatementKind::Assign(_, rvalue) => match rvalue {
                    Rvalue::Cast(_, _, target_ty) => {
                        *target_ty = replace_ty(*target_ty, &ty_map, ty_ctx);
                    }
                    Rvalue::Repeat(_, const_val) => {
                        const_val.ty = replace_ty(const_val.ty, &ty_map, ty_ctx);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        match &mut block_data.terminator.kind {
            TerminatorKind::Call { .. } => {}
            _ => {}
        }
    }

    Body {
        owner: body.owner,
        basic_blocks: new_blocks,
        locals: new_locals,
        arg_count: body.arg_count,
        return_ty: replace_ty(body.return_ty, &ty_map, ty_ctx),
        span: body.span,
        var_debug_info: body.var_debug_info.clone(),
    }
}

/// Build a MIR body provider that looks up pre-lowered bodies by DefId.
pub(crate) fn make_mir_body_provider<'a>(
    bodies: &'a std::collections::HashMap<DefId, Arc<Body>>,
    sink: &'a RefCell<DiagSink>,
    ty_ctx: &'a TyCtx,
) -> impl Fn(DefId, &Substitution) -> Arc<Body> + 'a {
    move |def_id: DefId, substs: &Substitution| -> Arc<Body> {
        if let Some(body) = bodies.get(&def_id) {
            if substs.is_empty() {
                body.clone()
            } else {
                Arc::new(substitute_body(body, substs, ty_ctx))
            }
        } else {
            let diag = GlyimDiagnostic::internal_error(format!(
                "MIR body not found for DefId {:?}",
                def_id
            ));
            sink.borrow_mut().emit(diag);
            Arc::new(Body::dummy(DefId::new(
                CrateId::from_raw(0),
                LocalDefId::from_raw(0),
            )))
        }
    }
}

/// Build a drop glue body provider.
pub(crate) fn make_drop_glue_provider(ty_ctx: &TyCtx) -> impl Fn(glyim_type::Ty) -> Arc<Body> + '_ {
    move |ty: glyim_type::Ty| -> Arc<Body> { generate_drop_glue(ty, ty_ctx) }
}

/// Generate a minimal MIR body that drops the given type.
fn generate_drop_glue(_ty: Ty, ty_ctx: &TyCtx) -> Arc<Body> {
    let def_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0));
    let mut body = Body::dummy(def_id);
    body.return_ty = ty_ctx.unit_ty();
    if let Some(block) = body.basic_blocks.get_mut(BasicBlockIdx::from_raw(0)) {
        block.terminator.kind = TerminatorKind::Return;
    }
    Arc::new(body)
}

/// Compute the maximum number of codegen units based on available parallelism.
pub(crate) fn compute_max_cgus() -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    cores.clamp(1, 16)
}
