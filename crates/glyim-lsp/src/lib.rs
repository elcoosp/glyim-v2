//! Language Server Protocol implementation for Glyim compiler.

pub mod state;
pub mod uri;
pub mod completion;
pub mod hover;
pub mod navigation;
pub mod diagnostics;
pub mod folding;
pub mod formatting;
pub mod code_action;
pub mod symbol_index;
pub mod reference_graph;
pub mod driver;
pub mod handler;
pub mod server;
pub mod database;  // Make database public

#[cfg(test)]
mod tests;

pub use state::LspState;
pub use symbol_index::{SymbolIndex, SymbolInfo, SymbolKind, DefinitionLocation, TypeSignature};
pub use reference_graph::{ReferenceGraph, Reference, ReferenceKind};
pub use database::AnalysisDatabase;
