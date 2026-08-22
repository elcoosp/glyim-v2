//! Language Server Protocol implementation for Glyim compiler.
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

/// code_action.
pub mod code_action;
/// completion.
pub mod completion;
/// database.
pub mod database;
/// dep_graph.
pub mod dep_graph;
/// diagnostics.
pub mod diagnostics;
/// driver.
pub mod driver;
/// folding.
pub mod folding;
/// formatting.
pub mod formatting;
/// goto_definition.
pub mod goto_definition;
/// handler.
pub mod handler;
/// hover.
pub mod hover;
/// navigation.
pub mod navigation;
/// reference_graph.
pub mod reference_graph;
/// rename.
pub mod rename;
/// server.
pub mod server;
/// state.
pub mod state;
/// symbol_index.
pub mod symbol_index;
/// uri.
pub mod uri; // Make database public

#[cfg(test)]
mod tests;

pub use database::AnalysisDatabase;
pub use database::{FileMap, SourceMap};
pub use goto_definition::goto_definition;
pub use reference_graph::{Reference, ReferenceGraph, ReferenceKind};
pub use state::LspState;
pub use symbol_index::{DefinitionLocation, SymbolIndex, SymbolInfo, SymbolKind, TypeSignature};
