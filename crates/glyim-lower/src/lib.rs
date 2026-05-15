//! THIR → MIR lowering + monomorphization.
pub mod discovery;
pub mod lower;
pub mod mono;

pub use lower::*;
pub use mono::*;
pub mod partition;

#[cfg(test)]
mod tests;
