//! Metaprogramming support: macro expansion, comptime evaluation coordination.
//!
//! For v0.1.0, this crate provides the expansion framework but
//! delegates actual evaluation to `glyim-mir-interp`.
//!
//! Uses `HygieneCtx` from `glyim-span` (the merged hygiene crate).

use glyim_core::interner::Name;
use glyim_diag::GlyimDiagnostic;
use glyim_span::{HygieneCtx, Mark, Span, Transparency};
use glyim_syntax::SyntaxNode;

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
}

impl<'a> Expander<'a> {
    pub fn new(hygiene: &'a mut HygieneCtx) -> Self {
        Self {
            hygiene,
            macros: Vec::new(),
        }
    }

    pub fn register_macro(&mut self, def: MacroDef) {
        self.macros.push(def);
    }

    #[tracing::instrument(level = "debug", skip(self, args, call_site))]
    pub fn expand(&mut self, name: Name, args: &SyntaxNode, call_site: Span) -> ExpansionResult {
        let _ = (args, call_site);

        if let Some(_def) = self.macros.iter().find(|m| m.name == name) {
            let _mark = Mark {
                expn_id: glyim_span::ExpnId::ROOT,
                transparency: Transparency::SemiTransparent,
            };
            // STUB: apply mark and expand
        }

        ExpansionResult {
            expanded: None,
            diagnostics: Vec::new(),
        }
    }

    #[tracing::instrument(level = "info", skip(self, root))]
    pub fn expand_crate(&mut self, root: &SyntaxNode) -> (SyntaxNode, Vec<GlyimDiagnostic>) {
        let _ = self.hygiene;
        (root.clone(), Vec::new())
    }
}
