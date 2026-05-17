use glyim_lower::LowerCtx;
use glyim_lower::LowerCtx;
use glyim_core::def_id::AdtId;
use glyim_lower::{AdtDef, AdtKind, LowerCtx};
use glyim_span::Span;
use glyim_type::TyCtx;
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpanOp {
    Push(Span),
    Pop,
}

pub struct MockLowerCtx<'a> {
    pub ty_ctx: &'a TyCtx,
    span_ops: RefCell<Vec<SpanOp>>,
}

impl<'a> MockLowerCtx<'a> {
    pub fn new(ty_ctx: &'a TyCtx) -> Self {
        Self {
            ty_ctx,
            span_ops: RefCell::new(Vec::new()),
        }
    }
    pub fn span_ops(&self) -> Vec<SpanOp> {
        self.span_ops.borrow().clone()
    }
    pub fn assert_spans_balanced(&self) {
        let depth: usize = self
            .span_ops
            .borrow()
            .iter()
            .fold(0, |acc: usize, op| match op {
                SpanOp::Push(_) => acc + 1,
                SpanOp::Pop => acc.saturating_sub(1),
            });
        assert_eq!(depth, 0, "Unbalanced span operations");
    }
}

impl LowerCtx for MockLowerCtx<'_> {
    fn ty_ctx(&self) -> &TyCtx {
        self.ty_ctx
    }
    fn adt_def(&self, _id: AdtId) -> AdtDef {
        AdtDef {
            variants: Vec::new(),
            kind: AdtKind::Struct,
        }
    }
    fn push_span(&self, span: Span) {
        self.span_ops.borrow_mut().push(SpanOp::Push(span));
    }
    fn pop_span(&self) {
        self.span_ops.borrow_mut().push(SpanOp::Pop);
    }
}
