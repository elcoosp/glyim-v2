use glyim_core::primitives::*;
use crate::ty::Ty;
use crate::substitution::Substitution;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FnSig {
    pub inputs: Substitution,
    pub output: Ty,
    pub c_variadic: bool,
    pub unsafety: Safety,
    pub abi: Abi,
}
