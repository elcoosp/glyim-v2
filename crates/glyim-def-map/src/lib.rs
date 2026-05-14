//! Module graph and lightweight definition map.

use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, LocalDefId};
use glyim_core::interner::{Interner, Name};
use glyim_core::path::{Path, PathKind};
use glyim_core::primitives::Visibility;
use glyim_diag::GlyimDiagnostic;
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use glyim_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

glyim_core::define_idx!(ModuleId);

#[derive(Clone, Debug)]
pub struct CrateDefMap {
    pub root: ModuleId,
    pub modules: IndexVec<ModuleId, ModuleData>,
    pub krate: CrateId,
    pub interner: Interner,
}

#[derive(Clone, Debug)]
pub struct ModuleData {
    pub parent: Option<ModuleId>,
    pub children: Vec<(Name, ModuleId)>,
    pub scope: ItemScope,
    pub origin: ModuleOrigin,
    pub span: Span,
}

impl ModuleData {
    pub fn resolve(&self, name: Name) -> Option<(LocalDefId, Visibility)> {
        self.scope.resolve(name)
    }
}

#[derive(Clone, Debug)]
pub enum ModuleOrigin {
    File { file_id: glyim_span::FileId },
    Inline { span: Span },
    CrateRoot,
}

#[derive(Clone, Debug, Default)]
pub struct ItemScope {
    pub types: Vec<(Name, LocalDefId, Visibility, Span)>,
    pub values: Vec<(Name, LocalDefId, Visibility, Span)>,
    pub macros: Vec<(Name, LocalDefId, Visibility, Span)>,
}

impl ItemScope {
    pub fn resolve(&self, name: Name) -> Option<(LocalDefId, Visibility)> {
        self.types
            .iter()
            .chain(self.values.iter())
            .find(|(n, _, _, _)| *n == name)
            .map(|(_, id, vis, _)| (*id, *vis))
    }

    pub fn declare(
        &mut self,
        name: Name,
        id: LocalDefId,
        vis: Visibility,
        span: Span,
        ns: Namespace,
    ) {
        let entry = (name, id, vis, span);
        match ns {
            Namespace::Types => self.types.push(entry),
            Namespace::Values => self.values.push(entry),
            Namespace::Macros => self.macros.push(entry),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Namespace {
    Types,
    Values,
    Macros,
}

#[derive(Clone, Debug, Default)]
pub struct PerNs {
    pub types: Option<(LocalDefId, Visibility)>,
    pub values: Option<(LocalDefId, Visibility)>,
    pub macros: Option<(LocalDefId, Visibility)>,
}

impl PerNs {
    pub fn is_none(&self) -> bool {
        self.types.is_none() && self.values.is_none() && self.macros.is_none()
    }
    pub fn from_types(id: LocalDefId, vis: Visibility) -> Self {
        Self {
            types: Some((id, vis)),
            values: None,
            macros: None,
        }
    }
}

pub struct Resolver<'a> {
    def_map: &'a CrateDefMap,
    module: ModuleId,
}

impl<'a> Resolver<'a> {
    pub fn new(def_map: &'a CrateDefMap, module: ModuleId) -> Self {
        Self { def_map, module }
    }

    pub fn resolve_path(&self, path: &Path) -> PerNs {
        let mut current_module = self.module;
        let start_idx = match path.kind {
            PathKind::Plain => 0,
            PathKind::SelfPath => 0,
            PathKind::Super(n) => {
                let mut module = current_module;
                for _ in 0..n {
                    if let Some(parent) = self.def_map.modules[module].parent {
                        module = parent;
                    } else {
                        break;
                    }
                }
                current_module = module;
                0
            }
            PathKind::Crate => {
                current_module = self.def_map.root;
                0
            }
        };

        for (i, segment) in path.segments.iter().enumerate().skip(start_idx) {
            let module_data = &self.def_map.modules[current_module];
            if i == path.segments.len() - 1 {
                let types = module_data
                    .scope
                    .types
                    .iter()
                    .find(|(n, _, _, _)| *n == segment.name);
                let values = module_data
                    .scope
                    .values
                    .iter()
                    .find(|(n, _, _, _)| *n == segment.name);
                let mut result = PerNs::default();
                if let Some(&(_, tid, tvis, _)) = types {
                    result.types = Some((tid, tvis));
                }
                if let Some(&(_, vid, vvis, _)) = values {
                    result.values = Some((vid, vvis));
                }
                return result;
            } else if let Some((_, child_id)) = module_data
                .children
                .iter()
                .find(|(n, _)| *n == segment.name)
            {
                current_module = *child_id;
            } else {
                return PerNs::default();
            }
        }
        PerNs::default()
    }

    pub fn def_map(&self) -> &CrateDefMap {
        self.def_map
    }

    pub fn module(&self) -> ModuleId {
        self.module
    }
}

#[tracing::instrument(skip(root))]
pub fn build_def_map(root: &SyntaxNode, krate: CrateId) -> (CrateDefMap, Vec<GlyimDiagnostic>) {
    let mut diagnostics = Vec::new();
    let mut modules: IndexVec<ModuleId, ModuleData> = IndexVec::new();
    let interner = Interner::default();
    let mut def_counter: u32 = 1;

    let root_module = modules.push(ModuleData {
        parent: None,
        children: Vec::new(),
        scope: ItemScope::default(),
        origin: ModuleOrigin::CrateRoot,
        span: Span::DUMMY,
    });

    collect_items(
        root,
        root_module,
        &mut modules,
        &mut diagnostics,
        &interner,
        &mut def_counter,
    );

    let def_map = CrateDefMap {
        root: root_module,
        modules,
        krate,
        interner,
    };
    (def_map, diagnostics)
}

/// Collect items from `parent_node` into `parent_module`.
/// Handles AST nodes for item kinds and token patterns for module declarations.
fn collect_items(
    parent_node: &SyntaxNode,
    parent_module: ModuleId,
    modules: &mut IndexVec<ModuleId, ModuleData>,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    interner: &Interner,
    def_counter: &mut u32,
) {
    let children: Vec<SyntaxElement> = parent_node.children_with_tokens().collect();
    let mut idx = 0;
    while idx < children.len() {
        let elem = &children[idx];

        // --- Module detection via token pattern: KwMod Ident Block ---
        if let Some(tok) = elem.as_token()
            && tok.kind() == SyntaxKind::KwMod
            && idx + 2 < children.len()
            && let Some(name_tok) = children[idx + 1].as_token()
            && name_tok.kind() == SyntaxKind::Ident
            && let Some(body_node) = children[idx + 2].as_node()
            && body_node.kind() == SyntaxKind::Block
        {
            let name = interner.intern(name_tok.text());
            let span = node_span(body_node);
            let child_module = modules.push(ModuleData {
                parent: Some(parent_module),
                children: Vec::new(),
                scope: ItemScope::default(),
                origin: ModuleOrigin::Inline { span },
                span,
            });
            modules[parent_module].children.push((name, child_module));
            collect_items(
                body_node,
                child_module,
                modules,
                diagnostics,
                interner,
                def_counter,
            );
            idx += 3;
            continue;
        }

        // --- Proper AST node handling ---
        if let Some(node) = elem.as_node() {
            let kind = node.kind();
            match kind {
                SyntaxKind::FnDef
                | SyntaxKind::StructDef
                | SyntaxKind::EnumDef
                | SyntaxKind::TraitDef
                | SyntaxKind::ImplDef
                | SyntaxKind::TypeAlias
                | SyntaxKind::ConstDef
                | SyntaxKind::StaticDef
                | SyntaxKind::ExternBlock => {
                    if let Some(ns) = namespace_for_kind(kind) {
                        if let Some(name_text) = extract_ident(node) {
                            let name = interner.intern(&name_text);
                            let vis = visibility_of_node(node);
                            let id = LocalDefId::from_raw(*def_counter);
                            *def_counter += 1;
                            let span = node_span(node);

                            let scope = &mut modules[parent_module].scope;

                            let existing = match ns {
                                Namespace::Types => {
                                    scope.types.iter().find(|(n, _, _, _)| *n == name)
                                }
                                Namespace::Values => {
                                    scope.values.iter().find(|(n, _, _, _)| *n == name)
                                }
                                Namespace::Macros => {
                                    scope.macros.iter().find(|(n, _, _, _)| *n == name)
                                }
                            };
                            if existing.is_some() {
                                diagnostics.push(GlyimDiagnostic::parse_error(
                                    span,
                                    format!("duplicate definition of `{}`", interner.resolve(name)),
                                ));
                            } else {
                                scope.declare(name, id, vis, span, ns);
                            }
                        } else {
                            tracing::warn!("STUB: item without name: {:?}", kind);
                        }
                    } else {
                        tracing::warn!("STUB: {:?} not yet implemented", kind);
                    }
                }
                SyntaxKind::Module => {
                    tracing::warn!("STUB: Module node not yet implemented");
                }
                SyntaxKind::Block => {
                    // Check if this Block starts with `mod` keyword.
                    let block_children: Vec<SyntaxElement> = node.children_with_tokens().collect();
                    if block_children.len() >= 3
                        && block_children[0]
                            .as_token()
                            .is_some_and(|t| t.kind() == SyntaxKind::KwMod)
                        && block_children[1]
                            .as_token()
                            .is_some_and(|t| t.kind() == SyntaxKind::Ident)
                    {
                        let name = interner.intern(block_children[1].as_token().unwrap().text());
                        let span = node_span(node);
                        let child_module = modules.push(ModuleData {
                            parent: Some(parent_module),
                            children: Vec::new(),
                            scope: ItemScope::default(),
                            origin: ModuleOrigin::Inline { span },
                            span,
                        });
                        modules[parent_module].children.push((name, child_module));
                        collect_items(
                            node,
                            child_module,
                            modules,
                            diagnostics,
                            interner,
                            def_counter,
                        );
                        idx += 1;
                        continue;
                    }
                    tracing::warn!("STUB: top-level Block ignored (not a module)");
                }
                SyntaxKind::UseDecl => {
                    tracing::warn!("STUB: {:?} not yet implemented", kind);
                }
                _ => {}
            }
        }
        idx += 1;
    }
}

/// Extract the text of the first `Ident` token child of `node`.
fn extract_ident(node: &SyntaxNode) -> Option<String> {
    if node.kind() == SyntaxKind::ImplDef {
        let offset = u32::from(node.text_range().start());
        return Some(format!("__impl_{}", offset));
    }
    for child in node.children_with_tokens() {
        if let Some(token) = child.as_token()
            && token.kind() == SyntaxKind::Ident
        {
            return Some(token.text().to_string());
        }
    }
    None
}

/// Determine the namespace for a given syntax kind.
fn namespace_for_kind(kind: SyntaxKind) -> Option<Namespace> {
    match kind {
        SyntaxKind::FnDef | SyntaxKind::ConstDef | SyntaxKind::StaticDef => Some(Namespace::Values),
        SyntaxKind::StructDef
        | SyntaxKind::EnumDef
        | SyntaxKind::TraitDef
        | SyntaxKind::ImplDef
        | SyntaxKind::TypeAlias
        | SyntaxKind::ExternBlock => Some(Namespace::Types),
        _ => None,
    }
}

/// Check whether this node has a `KwPub` token among its sibling tokens.
fn visibility_of_node(node: &SyntaxNode) -> Visibility {
    let mut prev = node.prev_sibling_or_token();
    while let Some(sibling) = prev {
        if let Some(token) = sibling.as_token() {
            if token.kind() == SyntaxKind::KwPub {
                return Visibility::Public;
            }
            if token.kind().is_trivia()
                || token.kind() == SyntaxKind::Comma
                || token.kind() == SyntaxKind::Semicolon
            {
                prev = token.prev_sibling_or_token();
                continue;
            }
            break;
        } else {
            break;
        }
    }
    Visibility::Inherited
}

/// Create a `Span` from a syntax node's text range.
fn node_span(node: &SyntaxNode) -> Span {
    let range = node.text_range();
    let lo = ByteIdx::from_raw(u32::from(range.start()));
    let hi = ByteIdx::from_raw(u32::from(range.end()));
    Span::new(FileId::BOGUS, lo, hi, SyntaxContext::ROOT)
}

#[cfg(test)]
mod tests;
