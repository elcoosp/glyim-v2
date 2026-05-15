use crate::lower::{AdtDef, AdtKind, AdtVariant, LowerCtx};
use glyim_core::def_id::AdtId;
use glyim_span::Span;
use glyim_type::TyCtx;

pub struct TestLowerCtx<'a> {
    pub ty_ctx: &'a TyCtx,
}

impl<'a> LowerCtx for TestLowerCtx<'a> {
    fn ty_ctx(&self) -> &TyCtx {
        self.ty_ctx
    }

    fn adt_def(&self, _id: AdtId) -> AdtDef {
        AdtDef {
            variants: vec![AdtVariant { fields: vec![] }],
            kind: AdtKind::Struct,
        }
    }

    fn push_span(&self, _span: Span) {}
    fn pop_span(&self) {}
}
