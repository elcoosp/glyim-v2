use glyim_core::interner::Name;
use crate::ty::{Ty, ConstVar};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Const {
    pub kind: ConstKind,
    pub ty: Ty,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ConstKind {
    Int(i128),
    Uint(u128),
    FloatBits(u64),
    Bool(bool),
    Char(char),
    String(Name),
    Unit,
    Infer(ConstVar),
    Param(ParamConst),
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParamConst {
    pub index: u32,
    pub name: Name,
}
