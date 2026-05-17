//! Language Server Protocol implementation for Glyim compiler.

pub mod code_action;
pub mod completion;
pub mod database;
pub mod diagnostics;
pub mod driver;
pub mod folding;
pub mod formatting;
pub mod handler;
pub mod hover;
pub mod navigation;
pub mod reference_graph;
pub mod server;
pub mod state;
pub mod symbol_index;
pub mod uri; // Make database public

#[cfg(test)]
mod tests;

pub use database::AnalysisDatabase;
pub use reference_graph::{Reference, ReferenceGraph, ReferenceKind};
pub use state::LspState;
pub use symbol_index::{DefinitionLocation, SymbolIndex, SymbolInfo, SymbolKind, TypeSignature};
