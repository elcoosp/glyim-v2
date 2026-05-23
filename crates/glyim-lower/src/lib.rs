//! THIR → MIR lowering + monomorphization.
pub mod discovery;
pub mod lower;
pub mod mono;

// New split modules (private to crate, exposed via lower)
pub mod builder;
pub mod lower_rvalue;
pub mod lower_terminator;

pub use lower::*;
// IteratorNextInfo is re-exported via pub use lower::*;
pub use mono::*;
pub mod partition;
pub mod polymorphize;
pub mod post_mono_checks;

#[cfg(test)]
mod tests;
