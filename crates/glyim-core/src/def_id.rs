use std::fmt;
use crate::arena::IdxLike;

macro_rules! define_def_id {
    ($($name:ident),* $(,)?) => {
        $(
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
            pub struct $name(u32);

            impl $name {
                pub fn from_raw(raw: u32) -> Self { Self(raw) }
                pub fn to_raw(self) -> u32 { self.0 }
                pub fn index(self) -> usize { self.0 as usize }
            }

            impl IdxLike for $name {
                fn from_raw(raw: u32) -> Self { Self(raw) }
                fn to_raw(self) -> u32 { self.0 }
            }
        )*
    };
}

define_def_id!(CrateId, LocalDefId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DefId { pub krate: CrateId, pub local_id: LocalDefId }

impl DefId {
    pub fn new(krate: CrateId, local_id: LocalDefId) -> Self { Self { krate, local_id } }
}

impl fmt::Display for CrateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "crate[{}]", self.0) }
}
impl fmt::Display for DefId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}::{}", self.krate, self.local_id.0) }
}

define_def_id!(AdtId, FnDefId, ClosureId, TraitDefId, ImplDefId, OpaqueTyId, TypeAliasId, ConstDefId, StaticDefId);
