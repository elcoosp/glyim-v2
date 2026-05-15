//! Module graph and lightweight definition map.

use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, LocalDefId};
use glyim_core::interner::{Interner, Name};
use glyim_core::path::{Path, PathKind, PathSegment};
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
    /// Every module has a unique `LocalDefId` so it can be referred to by `use` paths
    pub def_id: LocalDefId,
    /// Visibility of the module itself (`pub mod` or private)
    pub visibility: Visibility,
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

/// Helper to resolve a path that ends at a module, returning the ModuleId.
fn resolve_module_path_for_modules(
    modules: &IndexVec<ModuleId, ModuleData>,
    start_module: ModuleId,
    path: &Path,
) -> Option<ModuleId> {
    let mut current_module = start_module;
    let start_idx = match path.kind {
        PathKind::Plain => 0,
        PathKind::SelfPath => 0,
        PathKind::Super(n) => {
            let mut module = current_module;
            for _ in 0..n {
                if let Some(parent) = modules[module].parent {
                    module = parent;
                } else {
                    return None;
                }
            }
            current_module = module;
            0
        }
        PathKind::Crate => {
            current_module = modules[start_module]
                .parent
                .map(|p| p)
                .unwrap_or(start_module);
            while let Some(parent) = modules[current_module].parent {
                current_module = parent;
            }
            0
        }
    };

    for segment in path.segments.iter().skip(start_idx) {
        let module_data = &modules[current_module];
        if let Some((_, child_id)) = module_data
            .children
            .iter()
            .find(|(n, _)| *n == segment.name)
        {
            current_module = *child_id;
        } else {
            return None;
        }
    }
    Some(current_module)
}

fn import_all_public_for_modules(
    source: ModuleId,
    target: ModuleId,
    modules: &mut IndexVec<ModuleId, ModuleData>,
) {
    let source_scope = modules[source].scope.clone();

    for (name, id, vis, span) in source_scope.types {
        if vis == Visibility::Public {
            modules[target]
                .scope
                .declare(name, id, vis, span, Namespace::Types);
        }
    }
    for (name, id, vis, span) in source_scope.values {
        if vis == Visibility::Public {
            modules[target]
                .scope
                .declare(name, id, vis, span, Namespace::Values);
        }
    }
}

/// Extract a `Path` from a syntax node (UseTree or PathExpr).
fn extract_path_from_syntax(node: &SyntaxNode, interner: &Interner) -> Option<Path> {
    let mut segments: Vec<PathSegment> = Vec::new();
    let mut kind = PathKind::Plain;
    let mut super_count = 0u32;

    fn visit(
        n: &SyntaxNode,
        segments: &mut Vec<PathSegment>,
        kind: &mut PathKind,
        super_count: &mut u32,
        interner: &Interner,
    ) {
        if n.kind() == SyntaxKind::PathExpr {
            for child in n.children() {
                visit(&child, segments, kind, super_count, interner);
            }
            return;
        }

        for elem in n.children_with_tokens() {
            if let Some(token) = elem.as_token() {
                match token.kind() {
                    SyntaxKind::KwCrate => *kind = PathKind::Crate,
                    SyntaxKind::KwSelf => *kind = PathKind::SelfPath,
                    SyntaxKind::KwSuper => {
                        *super_count += 1;
                        *kind = PathKind::Super(*super_count);
                    }
                    SyntaxKind::Ident => {
                        let name = interner.intern(token.text());
                        segments.push(PathSegment { name });
                    }
                    _ => {}
                }
            } else if let Some(child_node) = elem.as_node() {
                visit(child_node, segments, kind, super_count, interner);
            }
        }
    }

    visit(node, &mut segments, &mut kind, &mut super_count, interner);

    if segments.is_empty() && super_count > 0 {
        return Some(Path {
            segments,
            kind: PathKind::Super(super_count),
        });
    }
    if segments.is_empty() && kind == PathKind::Crate {
        return Some(Path {
            segments,
            kind: PathKind::Crate,
        });
    }

    if !segments.is_empty() {
        Some(Path { segments, kind })
    } else {
        None
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
        def_id: LocalDefId::from_raw(0), // root gets id 0
        visibility: Visibility::Public,  // root is always public
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

fn process_use_decl(
    node: &SyntaxNode,
    parent_module: ModuleId,
    modules: &mut IndexVec<ModuleId, ModuleData>,
    interner: &Interner,
) {
    let use_tree = match node.children().find(|n| n.kind() == SyntaxKind::UseTree) {
        Some(t) => t,
        None => return,
    };
    process_use_tree(&use_tree, parent_module, modules, interner);
}

fn process_use_tree(
    node: &SyntaxNode,
    parent_module: ModuleId,
    modules: &mut IndexVec<ModuleId, ModuleData>,
    interner: &Interner,
) {
    // Check all children (nodes and tokens) for special markers
    let has_glob = node
        .children_with_tokens()
        .any(|e| e.kind() == SyntaxKind::Star);
    let has_nested = node
        .children_with_tokens()
        .any(|e| e.kind() == SyntaxKind::LBrace);
    let use_path_node = node.children().find(|n| n.kind() == SyntaxKind::UsePath);

    // Handle glob import: use std::io::*;
    if has_glob {
        if let Some(path_node) = use_path_node {
            let path = extract_path_from_syntax(&path_node, interner);
            if let Some(p) = path {
                if let Some(mod_id) = resolve_module_path_for_modules(modules, parent_module, &p) {
                    import_all_public_for_modules(mod_id, parent_module, modules);
                }
            }
        }
        return;
    }

    // Handle nested import: use std::io::{Read, Write};
    if has_nested {
        if let Some(path_node) = use_path_node {
            let path = extract_path_from_syntax(&path_node, interner);

            // Get the base module for the path before the braces
            let base_module = if let Some(p) = path {
                resolve_module_path_for_modules(modules, parent_module, &p)
            } else {
                None
            };

            // Process each item inside the braces
            for child in node.children() {
                if child.kind() == SyntaxKind::UseTree {
                    let inner_use_path = child.children().find(|n| n.kind() == SyntaxKind::UsePath);
                    if let Some(inner_path_node) = inner_use_path {
                        let inner_path = extract_path_from_syntax(&inner_path_node, interner);

                        if let (Some(base_mod), Some(inner_p)) = (base_module, inner_path)
                            && inner_p.segments.len() == 1
                        {
                            let name = inner_p.segments[0].name;

                            // Handle submodule import (e.g. `use std::io::{self, Read}` => `io` itself)
                            if let Some(child_mod_id) = modules[base_mod]
                                .children
                                .iter()
                                .find(|(n, _)| *n == name)
                                .map(|(_, id)| *id)
                            {
                                let module_data = &modules[child_mod_id];
                                let def_id = module_data.def_id;
                                let vis = module_data.visibility;
                                modules[parent_module].scope.declare(
                                    name,
                                    def_id,
                                    vis,
                                    node_span(&child),
                                    Namespace::Types,
                                );
                            }

                            // Now resolve types/values for the inner segment
                            let temp_def_map = CrateDefMap {
                                root: ModuleId::from_raw(0),
                                modules: modules.clone(),
                                krate: CrateId::from_raw(0),
                                interner: interner.clone(),
                            };
                            let resolver = Resolver::new(&temp_def_map, base_mod);
                            let per_ns = resolver.resolve_path(&inner_p);

                            if let Some((id, vis)) = per_ns.types {
                                modules[parent_module].scope.declare(
                                    name,
                                    id,
                                    vis,
                                    node_span(&child),
                                    Namespace::Types,
                                );
                            }
                            if let Some((id, vis)) = per_ns.values {
                                modules[parent_module].scope.declare(
                                    name,
                                    id,
                                    vis,
                                    node_span(&child),
                                    Namespace::Values,
                                );
                            }
                        }
                    }
                }
            }
        }
        return;
    }

    // Simple path: use foo::bar;
    if let Some(path_node) = use_path_node {
        let path = extract_path_from_syntax(&path_node, interner);
        if let Some(path) = path {
            // NEW: Handle module import (e.g. `use std::io;` where `io` is a module)
            if let Some(mod_id) = resolve_module_path_for_modules(modules, parent_module, &path) {
                let module_data = &modules[mod_id];
                let def_id = module_data.def_id;
                let vis = module_data.visibility;
                if let Some(name) = path.segments.last().map(|s| s.name) {
                    modules[parent_module].scope.declare(
                        name,
                        def_id,
                        vis,
                        node_span(node),
                        Namespace::Types,
                    );
                }
            }

            // Existing type/value resolution (unchanged)
            let temp_def_map = CrateDefMap {
                root: ModuleId::from_raw(0),
                modules: modules.clone(),
                krate: CrateId::from_raw(0),
                interner: interner.clone(),
            };
            let resolver = Resolver::new(&temp_def_map, parent_module);
            let per_ns = resolver.resolve_path(&path);

            let name = path.segments.last().map(|s| s.name);

            if let Some(name) = name {
                if let Some((id, vis)) = per_ns.types {
                    modules[parent_module].scope.declare(
                        name,
                        id,
                        vis,
                        node_span(node),
                        Namespace::Types,
                    );
                }
                if let Some((id, vis)) = per_ns.values {
                    modules[parent_module].scope.declare(
                        name,
                        id,
                        vis,
                        node_span(node),
                        Namespace::Values,
                    );
                }
                if let Some((id, vis)) = per_ns.macros {
                    modules[parent_module].scope.declare(
                        name,
                        id,
                        vis,
                        node_span(node),
                        Namespace::Macros,
                    );
                }
            }
        }
    }
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
                let vis = visibility_of_node(&child);
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
                        def_id: LocalDefId::from_raw(*def_counter),
                        visibility: vis,
                    });
                    *def_counter += 1;
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

            SyntaxKind::UseDecl => {
                process_use_decl(&child, parent_module, modules, interner);
            }

            // Any other node (e.g., inner items inside a block are not expected)
            _ => {}
        }
    }
}

/// Extract the name of an inline module from its `Module` node as a String.
fn extract_module_name(module_node: &SyntaxNode) -> String {
    for child in module_node.children_with_tokens() {
        if let Some(token) = child.as_token()
            && token.kind() == SyntaxKind::Ident
        {
            return token.text().to_string();
        }
    }
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
        if let Some(token) = child.as_token()
            && token.kind() == SyntaxKind::Ident
        {
            return token.text().to_string();
        }
    }
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
fn node_span(node: &SyntaxNode) -> Span {
    let range = node.text_range();
    let lo = ByteIdx::from_raw(u32::from(range.start()));
    let hi = ByteIdx::from_raw(u32::from(range.end()));
    Span::new(FileId::BOGUS, lo, hi, SyntaxContext::ROOT)
}

#[cfg(test)]
mod tests;
