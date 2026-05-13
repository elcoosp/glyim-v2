use crate::ty::BoundTyKind;
use crate::region::BoundRegionKind;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Binder<T> {
    pub value: T,
    pub bound_vars: Box<[BoundVariableKind]>,
}

impl<T> Binder<T> {
    pub fn bind(value: T, bound_vars: Box<[BoundVariableKind]>) -> Self {
        Self { value, bound_vars }
    }
    pub fn skip_binder(self) -> T { self.value }
    pub fn as_ref(&self) -> Binder<&T> {
        Binder { value: &self.value, bound_vars: self.bound_vars.clone() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BoundVariableKind {
    Ty(BoundTyKind),
    Region(BoundRegionKind),
    Const,
}
