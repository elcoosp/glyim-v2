use glyim_borrowck::BorrowckCtx;
use glyim_core::def_id::{AdtId, ConstDefId};
use glyim_hir::{CrateHir, ItemKind};
use glyim_lower::{AdtDef, AdtKind, AdtVariant, LowerCtx};
use glyim_mir::{Body, LocalDecl, LocalIdx, MirConst, MirConstKind};
use glyim_span::Span;
use glyim_type::TyCtx;
use std::cell::RefCell;
use std::collections::HashMap;
use tracing::warn;

pub(crate) struct PipelineLowerCtx<'a> {
    ty_ctx: &'a TyCtx,
    hir: &'a CrateHir,
    /// Evaluated const values (Part C: const value materialization), owned so
    /// the context does not borrow `TypeckResult` for its whole lifetime (the
    /// result is also moved/consumed elsewhere in the pipeline).
    const_values: HashMap<ConstDefId, glyim_const_eval::ConstValue>,
    span_stack: RefCell<Vec<Span>>,
}

impl<'a> PipelineLowerCtx<'a> {
    pub(crate) fn new(
        ty_ctx: &'a TyCtx,
        hir: &'a CrateHir,
        const_values: HashMap<ConstDefId, glyim_const_eval::ConstValue>,
    ) -> Self {
        PipelineLowerCtx {
            ty_ctx,
            hir,
            const_values,
            span_stack: RefCell::new(Vec::new()),
        }
    }
}

impl<'a> LowerCtx for PipelineLowerCtx<'a> {
    fn ty_ctx(&self) -> &TyCtx {
        self.ty_ctx
    }
    fn hir_body(&self, owner: glyim_core::def_id::LocalDefId) -> Option<&glyim_hir::Body> {
        // Iterate through body owners to find the one matching the given LocalDefId
        for (body_id, body_owner) in self.hir.body_owners.iter_enumerated() {
            if *body_owner == owner {
                return self.hir.bodies.get(body_id);
            }
        }
        None
    }
    fn adt_def(&self, id: AdtId) -> AdtDef {
        if let Some(adt_def) = self.ty_ctx.adt_def(id) {
            let variants = adt_def
                .variants
                .iter()
                .map(|variant| AdtVariant {
                    fields: variant.fields.iter().map(|field| field.ty).collect(),
                })
                .collect();
            let kind = match adt_def.kind {
                glyim_type::AdtKind::Struct => AdtKind::Struct,
                glyim_type::AdtKind::Enum => AdtKind::Enum,
                glyim_type::AdtKind::Union => AdtKind::Union,
            };
            return AdtDef { variants, kind };
        }

        let def_id = glyim_core::def_id::DefId::new(
            glyim_core::def_id::CrateId::from_raw(0),
            glyim_core::def_id::LocalDefId::from_raw(id.to_raw()),
        );
        let item_id = glyim_hir::ItemId::from_raw(def_id.local_id.to_raw());

        match self.hir.items.get(item_id) {
            Some(item) => match &item.kind {
                ItemKind::Struct(s) => {
                    let fields = s
                        .fields
                        .iter()
                        .map(|_field| self.ty_ctx.error_ty())
                        .collect();
                    AdtDef {
                        variants: vec![AdtVariant { fields }],
                        kind: AdtKind::Struct,
                    }
                }
                ItemKind::Enum(e) => {
                    let variants = e
                        .variants
                        .iter()
                        .map(|variant| AdtVariant {
                            fields: variant
                                .fields
                                .iter()
                                .map(|_f| self.ty_ctx.error_ty())
                                .collect(),
                        })
                        .collect();
                    AdtDef {
                        variants,
                        kind: AdtKind::Enum,
                    }
                }
                _ => {
                    warn!("ADT id {:?} resolved to non-struct/enum item", id);
                    AdtDef {
                        variants: Vec::new(),
                        kind: AdtKind::Struct,
                    }
                }
            },
            None => {
                warn!("ADT id {:?} not found in HIR items", id);
                AdtDef {
                    variants: Vec::new(),
                    kind: AdtKind::Struct,
                }
            }
        }
    }

    fn push_span(&self, span: Span) {
        self.span_stack.borrow_mut().push(span);
    }

    fn pop_span(&self) {
        self.span_stack.borrow_mut().pop();
    }

    fn const_value(
        &self,
        def_id: glyim_core::def_id::ConstDefId,
        _substs: glyim_type::Substitution,
    ) -> Option<glyim_mir::MirConst> {
        // Part C: const value materialization. Fold a `ConstRef` into a
        // concrete `MirConst` from the const-evaluated value produced by
        // typeck (stored in `TypeckResult::const_values`). Scalar constants
        // (integers, floats, bool, char, str, unit) fold fully; aggregate
        // constants (tuple/array/struct) and ranges fall back to `None` so the
        // caller emits a `ConstRef` (zero-initialized global) as before.
        let value = self.const_values.get(&def_id)?;
        let ty = self.ty_ctx.const_ty(def_id).unwrap_or_else(|| self.ty_ctx.error_ty());
        let kind = match value {
            glyim_const_eval::ConstValue::Int(v, _) => MirConstKind::Int(*v),
            glyim_const_eval::ConstValue::Uint(v, _) => MirConstKind::Uint(*v),
            glyim_const_eval::ConstValue::FloatBits(b, _) => MirConstKind::FloatBits(*b),
            glyim_const_eval::ConstValue::Bool(b) => MirConstKind::Bool(*b),
            glyim_const_eval::ConstValue::Char(c) => MirConstKind::Char(*c),
            glyim_const_eval::ConstValue::String(n) => MirConstKind::String(*n),
            glyim_const_eval::ConstValue::Unit => MirConstKind::Unit,
            glyim_const_eval::ConstValue::Tuple(_)
            | glyim_const_eval::ConstValue::Array(_)
            | glyim_const_eval::ConstValue::Struct(_)
            | glyim_const_eval::ConstValue::Range(..) => return None,
        };
        Some(MirConst {
            kind,
            ty,
            span: Span::DUMMY,
        })
    }
}

pub(crate) struct PipelineBorrowckCtx<'a> {
    ty_ctx: &'a TyCtx,
    body: &'a Body,
}

impl<'a> PipelineBorrowckCtx<'a> {
    pub(crate) fn new(ty_ctx: &'a TyCtx, body: &'a Body) -> Self {
        PipelineBorrowckCtx { ty_ctx, body }
    }
}

impl<'a> BorrowckCtx for PipelineBorrowckCtx<'a> {
    fn ty_ctx(&self) -> &TyCtx {
        self.ty_ctx
    }

    fn local_decl(&self, idx: LocalIdx) -> &LocalDecl {
        &self.body.locals[idx]
    }

    fn local_name(&self, idx: LocalIdx) -> String {
        format!("local_{}", idx.to_raw())
    }
}
