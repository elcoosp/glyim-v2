use crate::ty::RegionVid;
use glyim_core::interner::Name;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Region {
    Static,
    EarlyBound(EarlyBoundRegion),
    LateBound(DebruijnIndex, u32, BoundRegionKind),
    Var(RegionVid),
    Erased,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EarlyBoundRegion {
    pub index: u32,
    pub name: Name,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DebruijnIndex(pub u32);

impl DebruijnIndex {
    pub const INNERMOST: DebruijnIndex = DebruijnIndex(0);
    pub fn shifted_in(self) -> Self {
        DebruijnIndex(self.0 + 1)
    }
    pub fn shifted_out(self) -> Option<Self> {
        if self.0 > 0 {
            Some(DebruijnIndex(self.0 - 1))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BoundRegionKind {
    BrAnon(u32),
    BrNamed(Name),
    BrEnv,
}
