use glyim_borrowck::BorrowckCtx;
use glyim_core::def_id::AdtId;
use glyim_hir::{CrateHir, ItemKind};
use glyim_lower::{AdtDef, AdtKind, AdtVariant, LowerCtx};
use glyim_mir::{Body, LocalDecl, LocalIdx};
use glyim_span::Span;
use glyim_type::TyCtx;
use std::cell::RefCell;
use tracing::warn;

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
