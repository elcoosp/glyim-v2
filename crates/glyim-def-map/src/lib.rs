//! Module graph and lightweight definition map.

use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, LocalDefId};
use glyim_core::interner::{Interner, Name};
use glyim_core::path::{Path, PathKind};
use glyim_core::primitives::Visibility;
use glyim_diag::GlyimDiagnostic;
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use glyim_syntax::{SyntaxKind, SyntaxNode};

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
    File { file_id: FileId },
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

/// Collect items from a syntax node (SourceFile or Module node) into the given module.
/// For a Module node, the node itself is the container; its children (nodes only) are the items.
fn collect_items(
    node: &SyntaxNode,
    parent_module: ModuleId,
    modules: &mut IndexVec<ModuleId, ModuleData>,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    interner: &Interner,
    def_counter: &mut u32,
) {
    for child in node.children() {
        match child.kind() {
            // Inline module: `mod name { ... }`
            SyntaxKind::Module => {
                let name_str = extract_module_name(&child);
                let name = interner.intern(&name_str);
                let span = node_span(&child);
                let is_dup = modules[parent_module]
                    .children
                    .iter()
                    .any(|(n, _)| *n == name);
                if is_dup {
                    diagnostics.push(GlyimDiagnostic::parse_error(
                        span,
                        format!("duplicate module `{}`", interner.resolve(name)),
                    ));
                } else {
                    let child_module = modules.push(ModuleData {
                        parent: Some(parent_module),
                        children: Vec::new(),
                        scope: ItemScope::default(),
                        origin: ModuleOrigin::Inline { span },
                        span,
                    });
                    modules[parent_module].children.push((name, child_module));

                    // Recurse into the module node itself. Its children (nodes) are the items inside.
                    collect_items(
                        &child,
                        child_module,
                        modules,
                        diagnostics,
                        interner,
                        def_counter,
                    );
                }
            }

            // Items that go into the namespace
            SyntaxKind::FnDef
            | SyntaxKind::StructDef
            | SyntaxKind::EnumDef
            | SyntaxKind::TraitDef
            | SyntaxKind::ImplDef
            | SyntaxKind::TypeAlias
            | SyntaxKind::ConstDef
            | SyntaxKind::StaticDef
            | SyntaxKind::ExternBlock => {
                if let Some(ns) = namespace_for_kind(child.kind()) {
                    let name_str = extract_ident(&child);
                    let name = interner.intern(&name_str);
                    let vis = visibility_of_node(&child);
                    let id = LocalDefId::from_raw(*def_counter);
                    *def_counter += 1;
                    let span = node_span(&child);

                    let scope = &mut modules[parent_module].scope;
                    let existing = match ns {
                        Namespace::Types => scope.types.iter().any(|(n, _, _, _)| *n == name),
                        Namespace::Values => scope.values.iter().any(|(n, _, _, _)| *n == name),
                        Namespace::Macros => scope.macros.iter().any(|(n, _, _, _)| *n == name),
                    };
                    if existing {
                        diagnostics.push(GlyimDiagnostic::parse_error(
                            span,
                            format!("duplicate definition of `{}`", interner.resolve(name)),
                        ));
                    } else {
                        scope.declare(name, id, vis, span, ns);
                    }
                }
            }

            // Use declarations are ignored for now (test t24 expects no items)
            SyntaxKind::UseDecl => {
                // intentionally ignored
            }

            // Any other node (e.g., inner items inside a block are not expected)
            _ => {}
        }
    }
}

/// Extract the name of an inline module from its `Module` node as a String.
fn extract_module_name(module_node: &SyntaxNode) -> String {
    // The structure: Module node has children: KwMod, Ident, then items.
    for child in module_node.children_with_tokens() {
        if let Some(token) = child.as_token() {
            if token.kind() == SyntaxKind::Ident {
                return token.text().to_string();
            }
        }
    }
    // Fallback (should not happen for valid syntax)
    "__unnamed_module".to_string()
}

/// Extract the name of an item (e.g., function, struct) from its syntax node.
/// For `ImplDef`, we generate a synthetic unique name because impls have no inherent name.
fn extract_ident(node: &SyntaxNode) -> String {
    if node.kind() == SyntaxKind::ImplDef {
        let offset = u32::from(node.text_range().start());
        return format!("__impl_{}", offset);
    }
    for child in node.children_with_tokens() {
        if let Some(token) = child.as_token() {
            if token.kind() == SyntaxKind::Ident {
                return token.text().to_string();
            }
        }
    }
    // Fallback: use the kind (Debug) and offset
    let offset = u32::from(node.text_range().start());
    format!("__{:?}_anonymous_{}", node.kind(), offset)
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

/// Extract visibility by looking for a `KwPub` token among the node's preceding siblings.
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
/// Note: In a real compiler, you would have access to the actual `FileId`.
/// Here we use a dummy `FileId::BOGUS`; tests do not rely on the file id.
fn node_span(node: &SyntaxNode) -> Span {
    let range = node.text_range();
    let lo = ByteIdx::from_raw(u32::from(range.start()));
    let hi = ByteIdx::from_raw(u32::from(range.end()));
    Span::new(FileId::BOGUS, lo, hi, SyntaxContext::ROOT)
}

#[cfg(test)]
mod tests;
