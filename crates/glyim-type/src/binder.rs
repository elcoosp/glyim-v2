use crate::ty::{BoundTyKind, BoundRegionKind};
#[derive(Clone, Debug, PartialEq, Eq, Hash)] pub struct Binder<T> { pub value: T, pub bound_vars: Box<[BoundVariableKind]> }
#[derive(Clone, Debug, PartialEq, Eq, Hash)] pub enum BoundVariableKind { Ty(BoundTyKind), Region(BoundRegionKind), Const }
