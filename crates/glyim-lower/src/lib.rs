//! THIR → MIR lowering + monomorphization.
pub mod lower;
pub mod mono;

pub use lower::*;
pub use mono::*;

#[cfg(test)]
mod tests;
