//! Polymorphization: detect unused generic parameters and avoid
//! monomorphizing over them, reducing code size.

mod analyze;
mod dedup;
mod substitute;

// Re-export public API
pub use analyze::analyze_used_params;
pub use dedup::{compute_poly_item, deduplicate};
pub use substitute::polymorphize_substs;
