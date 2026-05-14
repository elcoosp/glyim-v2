use glyim_borrowck::BorrowckCtx;
use glyim_core::def_id::AdtId;
use glyim_hir::CrateHir;
use glyim_lower::{AdtDef, AdtKind, LowerCtx};
use glyim_mir::{Body, LocalDecl, LocalIdx};
use glyim_span::Span;
use glyim_type::{Ty, TyCtx};
use std::cell::RefCell;

/// Real LowerCtx used by the pipeline.
pub(crate) struct PipelineLowerCtx<'a> {
    ty_ctx: &'a TyCtx,
    span_stack: RefCell<Vec<Span>>,
}

impl<'a> PipelineLowerCtx<'a> {
    pub(crate) fn new(ty_ctx: &'a TyCtx, _hir: &CrateHir) -> Self {
        // For v0.1.0, we do not precompute ADT definitions; return empty AdtDef on demand.
        PipelineLowerCtx {
            ty_ctx,
            span_stack: RefCell::new(Vec::new()),
        }
    }
}

impl<'a> LowerCtx for PipelineLowerCtx<'a> {
    fn ty_ctx(&self) -> &TyCtx {
        self.ty_ctx
    }

    fn adt_def(&self, _id: AdtId) -> AdtDef {
        // STUB: return empty AdtDef. Real implementation would look up type information.
        AdtDef {
            variants: Vec::new(),
            kind: AdtKind::Struct,
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
        // STUB: no copy analysis yet
        false
    }
}
