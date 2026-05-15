use glyim_borrowck::BorrowckCtx;
use glyim_core::def_id::{AdtId, DefId};
use glyim_hir::{CrateHir, ItemKind};
use glyim_lower::{AdtDef, AdtKind, AdtVariant, LowerCtx};
use glyim_mir::{Body, LocalDecl, LocalIdx};
use glyim_span::Span;
use glyim_type::{Ty, TyCtx};
use std::cell::RefCell;
use tracing::warn;

/// Real LowerCtx used by the pipeline.
pub(crate) struct PipelineLowerCtx<'a> {
    ty_ctx: &'a TyCtx,
    hir: &'a CrateHir,
    span_stack: RefCell<Vec<Span>>,
}

impl<'a> PipelineLowerCtx<'a> {
    pub(crate) fn new(ty_ctx: &'a TyCtx, hir: &'a CrateHir) -> Self {
        PipelineLowerCtx {
            ty_ctx,
            hir,
            span_stack: RefCell::new(Vec::new()),
        }
    }
}

impl<'a> LowerCtx for PipelineLowerCtx<'a> {
    fn ty_ctx(&self) -> &TyCtx {
        self.ty_ctx
    }

    fn adt_def(&self, id: AdtId) -> AdtDef {
        let def_id = DefId::new(glyim_core::def_id::CrateId::from_raw(0), glyim_core::def_id::LocalDefId::from_raw(id.to_raw()));
        let item_id = glyim_hir::ItemId::from_raw(def_id.local_id.to_raw());

        match self.hir.items.get(item_id) {
            Some(item) => match &item.kind {
                ItemKind::Struct(s) => {
                    let fields: Vec<Ty> = s
                        .fields
                        .iter()
                        .map(|_f| {
                            // STUB: field type resolution not yet implemented;
                            // using error type until TyCtx provides a method to
                            // retrieve primitive types by name.
                            warn!("STUB: ADT field type not resolved; using error type");
                            self.ty_ctx.error_ty()
                        })
                        .collect();
                    AdtDef {
                        variants: vec![AdtVariant { fields }],
                        kind: AdtKind::Struct,
                    }
                }
                ItemKind::Enum(e) => {
                    let variants: Vec<AdtVariant> = e
                        .variants
                        .iter()
                        .map(|v| {
                            let fields: Vec<Ty> = v
                                .fields
                                .iter()
                                .map(|_f| {
                                    warn!("STUB: ADT field type not resolved; using error type");
                                    self.ty_ctx.error_ty()
                                })
                                .collect();
                            AdtVariant { fields }
                        })
                        .collect();
                    AdtDef {
                        variants,
                        kind: AdtKind::Enum,
                    }
                }
                _ => {
                    warn!("STUB: ADT id {:?} resolved to non-struct/enum item", id);
                    AdtDef {
                        variants: Vec::new(),
                        kind: AdtKind::Struct,
                    }
                }
            },
            None => {
                warn!("STUB: ADT id {:?} not found in HIR items", id);
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
}

/// Real BorrowckCtx used by the pipeline.
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

    fn is_copy(&self, _ty: Ty) -> bool {
        false
    }
}
