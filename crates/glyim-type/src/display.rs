use crate::ty::Ty;
pub trait TypeLookup {
    fn ty_kind(&self, _ty: Ty) -> &crate::ty::TyKind;
    fn error_ty(&self) -> Ty;
}
pub struct PrintTy<'a, L: TypeLookup>(pub Ty, pub &'a L);
pub struct DebugTy<'a, L: TypeLookup>(pub PrintTy<'a, L>);
