use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::interner::{Interner, Name};
use glyim_hir::{Body, BodyId, CrateHir, Expr};
use glyim_span::Span;
use glyim_type::Ty;

pub fn name(s: &str) -> Name {
    let interner = Interner::new();
    interner.intern(s)
}
pub fn make_single_body_hir(exprs: Vec<Expr>) -> (CrateHir, BodyId) {
    let mut hir = CrateHir {
        items: Default::default(),
        bodies: Default::default(),
        body_owners: Default::default(),
    };
    // Build IndexVec by pushing each expr
    let mut expr_vec = IndexVec::new();
    for expr in exprs {
        expr_vec.push(expr);
    }
    let body = Body {
        owner: LocalDefId::from_raw(0),
        exprs: expr_vec,
        pats: Default::default(),
        params: vec![],
        span: Span::DUMMY,
        expr_spans: Default::default(),
    let body_id = hir.bodies.push(body);
    (hir, body_id)
pub fn typeck_single_body(_hir: &CrateHir, _body_id: BodyId) -> crate::thir::Body {
    crate::thir::Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        return_ty: Ty::UNIT,
        stmts: vec![],
