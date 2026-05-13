use glyim_core::interner::Name;
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BoundRegionKind { BrAnon(u32), BrNamed(Name), BrEnv }
