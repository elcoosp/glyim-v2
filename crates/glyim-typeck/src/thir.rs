use glyim_core::def_id::DefId;
use glyim_span::Span;
#[derive(Clone, Debug)]
pub struct Body {
    pub owner: DefId,
    pub span: Span,
}
impl Body {
    pub fn dummy(owner: DefId) -> Self { Self { owner, span: Span::DUMMY } }
}
