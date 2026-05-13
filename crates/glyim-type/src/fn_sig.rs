use crate::ty::{Substitution, Ty};
#[derive(Clone, Debug, PartialEq, Eq, Hash)] pub struct FnSig { pub inputs: Substitution, pub output: Ty }
