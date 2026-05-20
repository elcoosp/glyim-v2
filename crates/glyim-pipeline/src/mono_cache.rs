#![allow(clippy::single_match)]
//! Mono item caching for the pipeline.

use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_diag::{DiagSink, GlyimDiagnostic};
use glyim_lower::mono::MonoItemData;
use glyim_mir::{
    BasicBlockData, BasicBlockIdx, Body, LocalIdx, Place, ProjectionElem, Rvalue, SourceInfo,
    Statement, StatementKind, Terminator, TerminatorKind,
};
use glyim_span::Span;
use glyim_type::{AdtKind, FieldIdx, GenericArg, ParamTy, Substitution, Ty, TyCtx, TyKind};
use std::cell::RefCell;
use std::sync::Arc;

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

pub(crate) fn substitute_body(body: &Body, substs: &Substitution, ty_ctx: &TyCtx) -> Body {
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

pub(crate) fn make_drop_glue_provider(ty_ctx: &TyCtx) -> impl Fn(glyim_type::Ty) -> Arc<Body> + '_ {
    move |ty: glyim_type::Ty| -> Arc<Body> { generate_drop_glue(ty, ty_ctx) }
}

pub(crate) fn generate_drop_glue(ty: Ty, ty_ctx: &TyCtx) -> Arc<Body> {
    let def_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0));
    let mut body = Body::dummy(def_id);
    body.return_ty = ty_ctx.unit_ty();

    let ptr_local = LocalIdx::from_raw(0);
    let place = Place::new(ptr_local);

    // Collect places to drop (simplified: just the whole value for now)
    let mut drop_places = Vec::new();
    collect_drop_places(ty, &place, ty_ctx, &mut drop_places);

    if drop_places.is_empty() {
        if let Some(block) = body.basic_blocks.get_mut(BasicBlockIdx::from_raw(0)) {
            block.terminator.kind = TerminatorKind::Return;
        }
        return Arc::new(body);
    }

    // Build a chain of basic blocks, each dropping one place.
    let start_block = BasicBlockIdx::from_raw(0);
    let return_block = BasicBlockIdx::from_raw(drop_places.len() as u32);

    for (i, drop_place) in drop_places.iter().enumerate() {
        let target = if i == drop_places.len() - 1 {
            return_block
        } else {
            BasicBlockIdx::from_raw((i + 1) as u32)
        };
        let terminator = Terminator {
            kind: TerminatorKind::Drop {
                place: drop_place.clone(),
                target,
                cleanup: None,
            },
            source_info: SourceInfo::new(Span::DUMMY),
        };
        let block_data = BasicBlockData {
            statements: vec![],
            terminator,
            is_cleanup: false,
        };
        if i == 0 {
            *body.basic_blocks.get_mut(start_block).unwrap() = block_data;
        } else {
            body.basic_blocks.push(block_data);
        }
    }

    // Add final return block
    body.basic_blocks.push(BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    });

    Arc::new(body)
}

fn collect_drop_places(ty: Ty, place: &Place, ty_ctx: &TyCtx, out: &mut Vec<Place>) {
    match ty_ctx.ty_kind(ty) {
        TyKind::Adt(adt_id, _) => {
            if let Some(adt_def) = ty_ctx.adt_def(*adt_id) {
                match adt_def.kind {
                    AdtKind::Struct => {
                        for (field_idx, _) in adt_def.variants[0].fields.iter().enumerate() {
                            let mut proj = place.projection.to_vec();
                            proj.push(ProjectionElem::Field(FieldIdx::from_raw(field_idx as u32)));
                            let field_place = Place {
                                local: place.local,
                                projection: proj.into_boxed_slice(),
                            };
                            // For simplicity, drop each field individually.
                            out.push(field_place);
                        }
                    }
                    AdtKind::Enum => {
                        // For enums, drop the whole place (simplified).
                        out.push(place.clone());
                    }
                    AdtKind::Union => {}
                }
            } else {
                out.push(place.clone());
            }
        }
        TyKind::Array(_, _) | TyKind::Slice(_) => {
            out.push(place.clone());
        }
        _ => {}
    }
}

pub(crate) fn compute_max_cgus() -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    cores.clamp(1, 16)
}
