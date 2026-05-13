use bitflags::bitflags;
bitflags! { #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub struct TypeFlags: u32 { const HAS_ERROR = 1 << 7; const HAS_DEPTH_OVERFLOW = 1 << 8; } }
pub fn compute_flags() -> TypeFlags { TypeFlags::empty() }
