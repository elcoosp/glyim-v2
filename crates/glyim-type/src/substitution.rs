use std::fmt;
use crate::ty::Ty;
use crate::region::Region;
use crate::const_val::Const;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Substitution {
    index: u32,
    len: u16,
}

impl Substitution {
    pub(crate) fn from_raw(index: u32, len: u16) -> Self { Self { index, len } }
    pub fn index(self) -> u32 { self.index }
    pub fn len(self) -> u16 { self.len }
    pub fn is_empty(self) -> bool { self.len == 0 }
}

impl fmt::Debug for Substitution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Substitution(index={}, len={})", self.index, self.len)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GenericArg {
    Ty(Ty),
    Lifetime(Region),
    Const(Const),
}
