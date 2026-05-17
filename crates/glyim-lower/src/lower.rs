use glyim_type::*;
use glyim_typeck::thir;
use glyim_span::Span;
use glyim_diag::GlyimDiagnostic;

#[derive(Clone, Debug)]
pub struct LowerResult {
    pub body: glyim_mir::Body,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

pub trait LowerCtx {
    fn ty_ctx(&self) -> &TyCtx;
    fn adt_def(&self, id: glyim_core::def_id::AdtId) -> AdtDef;
    fn push_span(&self, span: Span);
    fn pop_span(&self);
}

pub struct AdtDef {
    pub variants: Vec<AdtVariant>,
    pub kind: AdtKind,
}

pub struct AdtVariant {
    pub fields: Vec<Ty>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdtKind {
    Struct,
    Enum,
    Union,
}

pub fn lower_body(ctx: &dyn LowerCtx, thir: &thir::Body) -> LowerResult {
    let mut builder = crate::builder::MirBuilder::new(ctx, thir);
    builder.lower_body(thir);

    let mut body = glyim_mir::Body::dummy(builder.owner);
    body.basic_blocks = builder.basic_blocks;
    body.locals = builder.locals;
    body.arg_count = builder.arg_count;
    body.return_ty = builder.return_ty;
    body.span = builder.span;

    LowerResult {
        body,
        diagnostics: builder.diagnostics,
    }
}
