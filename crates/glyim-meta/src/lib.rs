//! Metaprogramming support: macro expansion, comptime evaluation coordination.
//!
//! For v0.1.0, this crate provides the expansion framework but
//! delegates actual evaluation to `glyim-mir-interp`.
//!
//! Uses `HygieneCtx` from `glyim-span` (the merged hygiene crate).
#![allow(missing_docs)]

use glyim_core::interner::{Interner, Name};
use glyim_diag::GlyimDiagnostic;
use glyim_span::{FileId, HygieneCtx, Span};
use glyim_syntax::SyntaxNode;
use glyim_vfs::Vfs;

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
    Env,
    Concat,
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
}

impl<'a> Expander<'a> {
    pub fn new(hygiene: &'a mut HygieneCtx) -> Self {
        Self {
            hygiene,
            macros: Vec::new(),
            interner: Interner::default(),
            current_file: FileId::BOGUS,
            vfs: None,
        }
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
        );
        let expanded = SyntaxNode::new_root(green);
        (expanded, diags)
    }
}

#[cfg(test)]
mod tests;
