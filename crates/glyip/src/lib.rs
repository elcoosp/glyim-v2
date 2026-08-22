//! Glyip — Cargo-like build tool for the Glyim compiler.
//!
//! Provides project scaffolding (`glyip new`), compilation (`glyip build`),
//! test execution (`glyip test`), and binary execution (`glyip run`),
//! with dependency resolution, incremental compilation via fingerprinting,
//! and crate caching.
// Stylistic clippy lints suppressed crate-wide (test-noise lints).
#![allow(
    clippy::cloned_ref_to_slice_refs,
    clippy::vec_init_then_push,
    clippy::assertions_on_constants,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::manual_c_str_literals,
    clippy::doc_lazy_continuation,
    clippy::empty_line_after_doc_comments,
    clippy::manual_strip,
    clippy::needless_range_loop,
    clippy::unnecessary_cast,
    clippy::clone_on_copy,
    clippy::mutable_key_type,
    clippy::only_used_in_recursion,
    clippy::let_unit_value,
    clippy::unnecessary_literal_unwrap,
    clippy::format_in_format_args,
    clippy::permissions_set_readonly_false,
    clippy::needless_lifetimes,
    clippy::collapsible_if
)]

pub mod cache;
pub mod commands;
pub mod config;
pub mod dep;
pub mod error;
pub mod fingerprint;
pub mod lockfile;
pub mod test_discovery;

pub use cache::Cache;
pub use commands::{
    BuildResult, NewResult, RunResult, TestResult, cmd_build, cmd_new, cmd_run, cmd_test,
};
pub use config::{BuildOptions, GlyipToml, NewOptions, RunOptions, TestOptions};
pub use dep::{CrateIndex, DependencyResolver, IndexEntry, RegistryClient};

#[cfg(feature = "registry")]
pub use dep::HttpRegistryClient;
pub use error::{GlyipError, GlyipResult};
pub use fingerprint::{Fingerprint, FingerprintStore};
pub use lockfile::{CrateSource, LockedCrate, Lockfile};

#[cfg(test)]
mod tests;
