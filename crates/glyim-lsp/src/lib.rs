//! Language Server Protocol implementation for Glyim compiler.
#![allow(missing_docs)]
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

pub mod code_action;
pub mod completion;
pub mod database;
pub mod dep_graph;
pub mod diagnostics;
pub mod driver;
pub mod folding;
pub mod formatting;
pub mod goto_definition;
pub mod handler;
pub mod hover;
pub mod navigation;
pub mod reference_graph;
pub mod rename;
pub mod server;
pub mod state;
pub mod symbol_index;
pub mod uri; // Make database public

#[cfg(test)]
mod tests;

pub use database::AnalysisDatabase;
pub use database::{FileMap, SourceMap};
pub use goto_definition::goto_definition;
pub use reference_graph::{Reference, ReferenceGraph, ReferenceKind};
pub use state::LspState;
pub use symbol_index::{DefinitionLocation, SymbolIndex, SymbolInfo, SymbolKind, TypeSignature};
