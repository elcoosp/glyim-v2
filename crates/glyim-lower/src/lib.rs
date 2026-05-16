//! THIR → MIR lowering + monomorphization.
pub mod discovery;
pub mod lower;
pub mod mono;

pub use lower::*;
pub use mono::*;
pub mod partition;
pub mod polymorphize;
pub mod post_mono_checks;

#[cfg(test)]
mod tests;
