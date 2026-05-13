use crate::substitution::Substitution;
use crate::ty::Ty;
use glyim_core::primitives::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FnSig {
    pub inputs: Substitution,
    pub output: Ty,
    pub c_variadic: bool,
    pub unsafety: Safety,
    pub abi: Abi,
}
