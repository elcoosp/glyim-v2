//! Metaprogramming support: macro expansion, comptime evaluation coordination.
//!
//! Uses `HygieneCtx` from `glyim-span` (the merged hygiene crate).
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

use glyim_core::interner::{Interner, Name};
use glyim_diag::GlyimDiagnostic;
use glyim_span::{FileId, HygieneCtx, Span};
use glyim_syntax::SyntaxNode;
use glyim_vfs::Vfs;

/// Re-serialize an expanded token stream into a parseable program.
///
/// `glyim_meta::Expander::expand_crate` reconstructs the tree from tokens and,
/// like all token streams, the reconstructed text carries no whitespace
/// between adjacent tokens (e.g. `fn` + `main` → `fnmain` when the source's
/// whitespace is not materialized as tree tokens). Re-serializing from the
/// *merged* text would lose token boundaries, so instead we walk the green
/// token stream and insert a single space between two adjacent word-like
/// tokens. That is a faithful serialization: rowan reparses it identically
/// (whitespace is trivia), and it avoids spurious token merging. This is the
/// bridge between the token-oriented expander and the text-oriented pipeline
/// (Phase 9.2).
pub fn join_tokens_with_spaces(token_stream: &SyntaxNode) -> String {
    let mut out = String::new();
    let mut prev_was_word = false;
    for event in token_stream.descendants_with_tokens() {
        if let rowan::NodeOrToken::Token(tok) = event {
            let text = tok.text();
            let is_word = text.chars().all(|c: char| c.is_alphanumeric() || c == '_')
                && !text.is_empty();
            if prev_was_word && is_word {
                out.push(' ');
            }
            out.push_str(text);
            prev_was_word = is_word;
        }
    }
    out
}

mod expander;

#[derive(Clone, Debug)]
pub enum MacroKind {
    Declarative { name: Name },
    Proc { name: Name },
    Builtin { name: Name, handler: BuiltinMacro },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinMacro {
    File,
    Line,
    Column,
    Include,
    IncludeStr,
    IncludeBytes,
    Env,
    OptionEnv,
    Concat,
    ConcatIdents,
    Stringify,
}

#[derive(Clone, Debug)]
pub struct MacroDef {
    pub name: Name,
    pub kind: MacroKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct ExpansionResult {
    pub expanded: Option<SyntaxNode>,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

pub struct Expander<'a> {
    hygiene: &'a mut HygieneCtx,
    macros: Vec<MacroDef>,
    interner: Interner,
    /// File id of the source currently being expanded. Used to anchor
    /// `file!` / `line!` / `column!` / `include!` to the real source when the
    /// caller supplies one (defaults to `FileId::BOGUS`).
    current_file: FileId,
    /// Optional virtual file system, used to resolve `include!` paths relative
    /// to the calling file and to compute real `line!`/`column!` offsets.
    vfs: Option<&'a Vfs>,
    /// Optional registry of procedural macros (Phase 9.2). When set, a
    /// `MacroKind::Proc` invocation is delegated to the registered function.
    proc_registry: Option<&'a glyim_proc_macro::Registry>,
}

impl<'a> Expander<'a> {
    pub fn new(hygiene: &'a mut HygieneCtx) -> Self {
        Self {
            hygiene,
            macros: Vec::new(),
            interner: Interner::default(),
            current_file: FileId::BOGUS,
            vfs: None,
            proc_registry: None,
        }
    }

    /// Provide a procedural-macro [`Registry`](glyim_proc_macro::Registry) so
    /// `MacroKind::Proc` invocations are dispatched to their registered
    /// expansion functions (Phase 9.2). Returns `&mut Self` for chaining.
    pub fn with_proc_registry(&mut self, registry: Option<&'a glyim_proc_macro::Registry>) -> &mut Self {
        self.proc_registry = registry;
        self
    }

    /// Set the `FileId` of the source being expanded. Enables `file!`,
    /// `line!`, `column!`, and `include!` to resolve against the real file.
    pub fn set_source_file(&mut self, file_id: FileId) {
        self.current_file = file_id;
    }

    /// Provide a virtual file system so `include!` / `line!` / `column!` /
    /// `file!` resolve against real source paths and contents.
    pub fn set_vfs(&mut self, vfs: &'a Vfs) {
        self.vfs = Some(vfs);
    }

    pub fn register_macro(&mut self, def: MacroDef) {
        self.macros.push(def);
    }

    /// Returns a reference to the interner used by this expander.
    ///
    /// Use this when creating `Name` values for `MacroDef` registration
    /// to ensure names match during expansion lookups.
    pub fn interner(&self) -> &Interner {
        &self.interner
    }

    #[tracing::instrument(level = "debug", skip(self, args, call_site))]
    pub fn expand(&mut self, name: Name, args: &SyntaxNode, call_site: Span) -> ExpansionResult {
        let (green_opt, diags) = expander::expand_macro_invocation(
            name,
            args,
            call_site,
            self.hygiene,
            &self.macros,
            &self.interner,
            self.current_file,
            self.vfs,
            0,
            self.proc_registry,
        );
        let expanded = green_opt.map(SyntaxNode::new_root);
        ExpansionResult {
            expanded,
            diagnostics: diags,
        }
    }

    #[tracing::instrument(level = "info", skip(self, root))]
    pub fn expand_crate(&mut self, root: &SyntaxNode) -> (SyntaxNode, Vec<GlyimDiagnostic>) {
        let (green, diags) = expander::expand_crate(
            root,
            &mut self.interner,
            self.hygiene,
            &self.macros,
            self.current_file,
            self.vfs,
            self.proc_registry,
        );
        let expanded = SyntaxNode::new_root(green);
        (expanded, diags)
    }
}

#[cfg(test)]
mod tests;
