#![allow(clippy::single_match)]
//! Mono item caching for the pipeline.

use glyim_core::Mutability;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_diag::{DiagSink, GlyimDiagnostic};
use glyim_lower::mono::MonoItemData;
use glyim_mir::{
    BasicBlockData, BasicBlockIdx, Body, LocalDecl, LocalIdx, Operand, Place, ProjectionElem,
    Rvalue, SourceInfo, Statement, StatementKind, SwitchTargets, Terminator, TerminatorKind,
    VariantIdx,
};
use glyim_span::Span;
use glyim_type::{
    AdtDef, AdtKind, FieldIdx, GenericArg, ParamTy, Substitution, Ty, TyCtx, TyCtxMut, TyKind,
};
use std::cell::RefCell;
use std::collections::HashSet;
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

/// Build a pipeline mono cache, applying polymorphization deduplication first.
///
/// Call this instead of `PipelineMonoCache::from_items` directly so that polymorphization
/// is actually integrated into the pipeline.
pub(crate) fn build_mono_cache(
    ctx: &mut glyim_lower::MonoCtx,
    ty_ctx: &mut TyCtxMut,
) -> PipelineMonoCache {
    ctx.polymorphize_and_deduplicate(ty_ctx);
    PipelineMonoCache::from_items(ctx.items())
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

/// Create a drop-glue provider that can generate recursive drop glue for ADTs.
///
/// The provider only requires `&TyCtx` (immutable) so it fits the existing pipeline
/// call site.  Enum discriminant locals currently use `error_ty()` as a placeholder
/// type; once `TyCtx` grows `u8_ty()` / `u16_ty()` accessors they can be swapped in.
pub(crate) fn make_drop_glue_provider(ty_ctx: &TyCtx) -> impl Fn(Ty) -> Arc<Body> + '_ {
    move |ty: Ty| -> Arc<Body> { generate_drop_glue(ty, ty_ctx) }
}

pub(crate) fn generate_drop_glue(ty: Ty, ty_ctx: &TyCtx) -> Arc<Body> {
    let def_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0));
    let mut body = Body::dummy(def_id);
    body.return_ty = ty_ctx.unit_ty();

    let ptr_local = LocalIdx::from_raw(0);
    let place = Place::new(ptr_local);

    // Fast path: types that do not need dropping get a single `Return`.
    if !type_needs_drop(ty, ty_ctx, &mut HashSet::new()) {
        set_return_terminator(&mut body, BasicBlockIdx::from_raw(0));
        return Arc::new(body);
    }

    match ty_ctx.ty_kind(ty) {
        TyKind::Adt(adt_id, _) => {
            if let Some(adt_def) = ty_ctx.adt_def(*adt_id) {
                match adt_def.kind {
                    AdtKind::Struct => {
                        generate_struct_drop_glue(&mut body, &place, adt_def, ty_ctx);
                    }
                    AdtKind::Enum => {
                        generate_enum_drop_glue(&mut body, &place, adt_def, ty_ctx);
                    }
                    AdtKind::Union => {
                        // Unions do not have automatic drop glue; the user is responsible
                        // for unsafe union manipulation.
                        set_return_terminator(&mut body, BasicBlockIdx::from_raw(0));
                    }
                }
            } else {
                set_return_terminator(&mut body, BasicBlockIdx::from_raw(0));
            }
        }
        TyKind::Array(_elem_ty, _) | TyKind::Slice(_elem_ty) => {
            // Arrays and slices are dropped as a whole; the runtime or a later
            // loop-generation pass can expand this to per-element drops if required.
            // The collector will still enqueue `DropGlue` for the element type when
            // it scans the terminator, so nested ADT elements are handled.
            set_return_terminator(&mut body, BasicBlockIdx::from_raw(0));
        }
        _ => {
            set_return_terminator(&mut body, BasicBlockIdx::from_raw(0));
        }
    }

    Arc::new(body)
}

fn set_return_terminator(body: &mut Body, block: BasicBlockIdx) {
    if let Some(block_data) = body.basic_blocks.get_mut(block) {
        block_data.terminator.kind = TerminatorKind::Return;
    }
}

// ---------------------------------------------------------------------------
// type_needs_drop
// ---------------------------------------------------------------------------

/// Determine whether a type needs drop glue.
///
/// Returns `false` for primitive types, references, and types that have already
/// been visited (prevents infinite recursion on recursive ADTs such as linked lists).
fn type_needs_drop(ty: Ty, ty_ctx: &TyCtx, visited: &mut HashSet<Ty>) -> bool {
    if !visited.insert(ty) {
        return false;
    }

    match ty_ctx.ty_kind(ty) {
        TyKind::Adt(adt_id, _) => {
            if let Some(adt_def) = ty_ctx.adt_def(*adt_id) {
                match adt_def.kind {
                    AdtKind::Union => false,
                    _ => adt_def.variants.iter().any(|v| {
                        v.fields
                            .iter()
                            .any(|f| type_needs_drop(f.ty, ty_ctx, visited))
                    }),
                }
            } else {
                false
            }
        }
        TyKind::Array(elem_ty, _) | TyKind::Slice(elem_ty) => {
            type_needs_drop(*elem_ty, ty_ctx, visited)
        }
        TyKind::Tuple(substs) => ty_ctx
            .substitution_args(*substs)
            .iter()
            .any(|arg| match *arg {
                GenericArg::Ty(t) => type_needs_drop(t, ty_ctx, visited),
                _ => false,
            }),
        TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => false,
        TyKind::FnPtr(_) | TyKind::FnDef(_, _) | TyKind::Closure(_, _) => false,
        TyKind::Never
        | TyKind::Unit
        | TyKind::Bool
        | TyKind::Int(_)
        | TyKind::Uint(_)
        | TyKind::Float(_)
        | TyKind::Char
        | TyKind::String => false,
        TyKind::Infer(_) | TyKind::Error => false,
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Struct drop glue
// ---------------------------------------------------------------------------

fn generate_struct_drop_glue(body: &mut Body, place: &Place, adt_def: &AdtDef, ty_ctx: &TyCtx) {
    let fields_to_drop: Vec<_> = adt_def.variants[0]
        .fields
        .iter()
        .enumerate()
        .filter(|(_, f)| type_needs_drop(f.ty, ty_ctx, &mut HashSet::new()))
        .map(|(i, _)| i)
        .collect();

    if fields_to_drop.is_empty() {
        set_return_terminator(body, BasicBlockIdx::from_raw(0));
        return;
    }

    let return_bb = body.basic_blocks.push(BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    }));

    let mut next_target = return_bb;

    // Build the tail of the chain in reverse order (last field -> second field).
    for field_idx in fields_to_drop.iter().skip(1).rev() {
        let mut proj = place.projection.to_vec();
        proj.push(ProjectionElem::Field(FieldIdx::from_raw(*field_idx as u32)));
        let field_place = Place {
            local: place.local,
            projection: proj.into_boxed_slice(),
        };

        let bb = body.basic_blocks.push(BasicBlockData::new(Terminator {
            kind: TerminatorKind::Drop {
                place: field_place,
                target: next_target,
                cleanup: None,
            },
            source_info: SourceInfo::new(Span::DUMMY),
        }));
        next_target = bb;
    }

    // The first field drop re-uses basic block 0.
    let first_field_idx = fields_to_drop[0];
    let mut proj = place.projection.to_vec();
    proj.push(ProjectionElem::Field(FieldIdx::from_raw(
        first_field_idx as u32,
    )));
    let field_place = Place {
        local: place.local,
        projection: proj.into_boxed_slice(),
    };

    if let Some(block0) = body.basic_blocks.get_mut(BasicBlockIdx::from_raw(0)) {
        block0.statements.clear();
        block0.terminator = Terminator {
            kind: TerminatorKind::Drop {
                place: field_place,
                target: next_target,
                cleanup: None,
            },
            source_info: SourceInfo::new(Span::DUMMY),
        };
    }
}

// ---------------------------------------------------------------------------
// Enum drop glue
// ---------------------------------------------------------------------------

fn generate_enum_drop_glue(body: &mut Body, place: &Place, adt_def: &AdtDef, ty_ctx: &TyCtx) {
    let variants = &adt_def.variants;

    // Block that all variant-specific drop chains fall through to.
    let return_bb = body.basic_blocks.push(BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    }));

    // TODO: once TyCtx exposes u8_ty() / u16_ty(), use the proper width here.
    let discr_ty = ty_ctx.error_ty();

    let discr_local = body.locals.push(LocalDecl {
        ty: discr_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    let discr_place = Place::new(discr_local);

    // Build a drop-chain entry block for every variant.
    let mut variant_entry_blocks = Vec::new();

    for (variant_idx, variant) in variants.iter().enumerate() {
        let fields_to_drop: Vec<_> = variant
            .fields
            .iter()
            .enumerate()
            .filter(|(_, f)| type_needs_drop(f.ty, ty_ctx, &mut HashSet::new()))
            .map(|(i, _)| i)
            .collect();

        if fields_to_drop.is_empty() {
            variant_entry_blocks.push(return_bb);
            continue;
        }

        let mut next_target = return_bb;

        // Reverse chain for all fields except the first.
        for field_idx in fields_to_drop.iter().skip(1).rev() {
            let mut proj = place.projection.to_vec();
            proj.push(ProjectionElem::Downcast(VariantIdx::from_raw(
                variant_idx as u32,
            )));
            proj.push(ProjectionElem::Field(FieldIdx::from_raw(*field_idx as u32)));
            let field_place = Place {
                local: place.local,
                projection: proj.into_boxed_slice(),
            };

            let bb = body.basic_blocks.push(BasicBlockData::new(Terminator {
                kind: TerminatorKind::Drop {
                    place: field_place,
                    target: next_target,
                    cleanup: None,
                },
                source_info: SourceInfo::new(Span::DUMMY),
            }));
            next_target = bb;
        }

        // First field for this variant gets its own block.
        let first_field_idx = fields_to_drop[0];
        let mut proj = place.projection.to_vec();
        proj.push(ProjectionElem::Downcast(VariantIdx::from_raw(
            variant_idx as u32,
        )));
        proj.push(ProjectionElem::Field(FieldIdx::from_raw(
            first_field_idx as u32,
        )));
        let field_place = Place {
            local: place.local,
            projection: proj.into_boxed_slice(),
        };

        let entry_bb = body.basic_blocks.push(BasicBlockData::new(Terminator {
            kind: TerminatorKind::Drop {
                place: field_place,
                target: next_target,
                cleanup: None,
            },
            source_info: SourceInfo::new(Span::DUMMY),
        }));
        variant_entry_blocks.push(entry_bb);
    }

    // Assemble SwitchInt in block 0.
    let branches: Vec<_> = variant_entry_blocks
        .iter()
        .enumerate()
        .map(|(i, bb)| (i as u128, *bb))
        .collect();
    let otherwise = return_bb;
    let switch_targets = SwitchTargets::new(branches.into_boxed_slice(), otherwise);

    if let Some(block0) = body.basic_blocks.get_mut(BasicBlockIdx::from_raw(0)) {
        block0.statements.clear();
        block0.statements.push(Statement {
            kind: StatementKind::Assign(discr_place.clone(), Rvalue::Discriminant(place.clone())),
            source_info: SourceInfo::new(Span::DUMMY),
        });
        block0.terminator = Terminator {
            kind: TerminatorKind::SwitchInt {
                discr: Operand::Copy(discr_place),
                switch_ty: discr_ty,
                targets: switch_targets,
            },
            source_info: SourceInfo::new(Span::DUMMY),
        };
    }
}

pub(crate) fn compute_max_cgus() -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    cores.clamp(1, 16)
}
