use glyim_core::def_id::AdtId;
use glyim_lower::{AdtDef, AdtKind, LowerCtx};
use glyim_span::Span;
use glyim_type::TyCtx;
use std::cell::RefCell;

pub struct MockLowerCtx<'a> {
    ty_ctx: &'a TyCtx,
    span_stack: RefCell<Vec<Span>>,
}

impl<'a> MockLowerCtx<'a> {
    pub fn new(ty_ctx: &'a TyCtx) -> Self {
        Self {
            ty_ctx,
            span_stack: RefCell::new(Vec::new()),
        }
    }
}

impl<'a> LowerCtx for MockLowerCtx<'a> {
    fn ty_ctx(&self) -> &TyCtx {
        self.ty_ctx
    }

    fn adt_def(&self, _id: AdtId) -> AdtDef {
        AdtDef {
            variants: vec![],
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
