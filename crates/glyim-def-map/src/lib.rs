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
fn resolve_module_path(def_map: &CrateDefMap, start_module: ModuleId, path: &Path) -> Option<ModuleId> {
    eprintln!("[DEBUG] resolve_module_path called");
    let mut current_module = start_module;
    let start_idx = match path.kind {
        PathKind::Plain => 0,
        PathKind::SelfPath => 0,
        PathKind::Super(n) => {
            let mut module = current_module;
            for _ in 0..n {
                if let Some(parent) = def_map.modules[module].parent {
                    module = parent;
                } else {
                    return None;
                }
            }
            current_module = module;
            0
        }
        PathKind::Crate => {
            current_module = def_map.root;
            0
        }
    };

    for segment in path.segments.iter().skip(start_idx) {
        let module_data = &def_map.modules[current_module];
        // Check children for the next module segment
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

/// Extract a `Path` from a syntax node (UseTree or PathExpr).
/// Recursively collects segments to handle nested structures.
/// Stops at `UseTreeList` (braces) to avoid mixing prefix with list items.
fn extract_path_from_syntax(node: &SyntaxNode, interner: &Interner) -> Option<Path> {
    eprintln!("[DEBUG] extract_path_from_syntax called");
    eprintln!("[DEBUG] extract_path_from_syntax: node kind = {:?}", node.kind());

    let has_braces = node.children().any(|n| n.kind() == SyntaxKind::LBrace);
    eprintln!("[DEBUG] has_braces = {}", has_braces);

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
                        eprintln!("[DEBUG]   Found Ident: {}", token.text());
                        segments.push(PathSegment { name });
                    }
                    _ => {}
                }
            } else if let Some(child_node) = elem.as_node() {
                // Do not recurse into nested UseTrees (e.g. inner items of a brace list)
                if child_node.kind() == SyntaxKind::UseTree {
                    continue;
                }
                visit(child_node, segments, kind, super_count, interner);
            }
        }
    }

    visit(node, &mut segments, &mut kind, &mut super_count, interner);

    eprintln!("[DEBUG]   Raw segments count: {}", segments.len());

    // Heuristic: if braces present, pop the last segment (likely the first item in the list)
    if has_braces && !segments.is_empty() {
        segments.pop();
        eprintln!("[DEBUG]   Popped segment. New count: {}", segments.len());
    }

    if segments.is_empty() && super_count > 0 {
        return Some(Path { segments, kind: PathKind::Super(super_count) });
    }
    if segments.is_empty() && kind == PathKind::Crate {
        return Some(Path { segments, kind: PathKind::Crate });
    }

    if !segments.is_empty() {
        eprintln!("[DEBUG]   Returning Path with segments: {:?}", segments.iter().map(|s| interner.resolve(s.name)).collect::<Vec<_>>());
        Some(Path { segments, kind })
    } else {
        eprintln!("[DEBUG]   Returning None");
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
    });

    // We create the CrateDefMap early so we can pass it to helpers
    let mut def_map = CrateDefMap {
        root: root_module,
        modules,
        krate,
        interner,
    };

    collect_items(
        root,
        root_module,
        &mut def_map,
        &mut diagnostics,
        &mut def_counter,
    );

    (def_map, diagnostics)
}

/// Process a `UseDecl` node.
fn process_use_decl(
    node: &SyntaxNode,
    parent_module: ModuleId,
    def_map: &mut CrateDefMap,
    diagnostics: &mut Vec<GlyimDiagnostic>,
) {
    eprintln!("[DEBUG] process_use_decl called");
    // UseDecl contains a UseTree
    let use_tree = match node.children().find(|n| n.kind() == SyntaxKind::UseTree) {
        Some(t) => t,
        None => return,
    };

    process_use_tree(&use_tree, parent_module, def_map, diagnostics);
}

/// Recursively process a `UseTree`.
fn process_use_tree(
    node: &SyntaxNode,
    parent_module: ModuleId,
    def_map: &mut CrateDefMap,
    _diagnostics: &mut Vec<GlyimDiagnostic>,
) {
    eprintln!("[DEBUG] process_use_tree called");
    // Check for nested use trees (braces)
    if node.children().any(|n| n.kind() == SyntaxKind::LBrace) {
        // Find the path prefix in this UseTree (e.g. `a` in `a::{b, c}`)
        eprintln!("[DEBUG] Calling extract_path_from_syntax");
        let path = extract_path_from_syntax(node, &def_map.interner);
        let base_module = if let Some(p) = path {
            resolve_module_path(def_map, parent_module, &p)
        } else {
            None
        };

        // Process each inner UseTree
        for child in node.children() {
            if child.kind() == SyntaxKind::UseTree {
                let is_glob = child.children().any(|n| n.kind() == SyntaxKind::Star);
                let inner_path = extract_path_from_syntax(&child, &def_map.interner);

                if let (Some(base_mod), Some(inner_p)) = (base_module, inner_path) {
                    if inner_p.segments.len() == 1 {
                        let name = inner_p.segments[0].name;
                        // Clone scope to avoid holding immutable borrow while mutating parent
                        let base_scope = def_map.modules[base_mod].scope.clone();
                        if let Some((id, vis)) = base_scope.resolve(name) {
                            let ns = determine_namespace(id, &base_scope);
                            def_map.modules[parent_module].scope.declare(
                                name, id, vis, node_span(&child),
                                ns
                            );
                        }
                    }
                } else if is_glob {
                     // Handle `a::*` inside braces if base_module exists
                     if let Some(base_mod) = base_module {
                         import_all_public(base_mod, parent_module, def_map);
                     }
                }
            }
        }
        return;
    }

    // Check for glob import
    if node.children().any(|n| n.kind() == SyntaxKind::Star) {
        eprintln!("[DEBUG] Calling extract_path_from_syntax");
        let path = extract_path_from_syntax(node, &def_map.interner);
        if let Some(p) = path {
            if let Some(mod_id) = resolve_module_path(def_map, parent_module, &p) {
                import_all_public(mod_id, parent_module, def_map);
            }
        }
        return;
    }

    // Simple path import: `use a::b::c`
    eprintln!("[DEBUG] Calling extract_path_from_syntax");
        let path = extract_path_from_syntax(node, &def_map.interner);
    if let Some(path) = path {
        let resolver = Resolver::new(def_map, parent_module);
        let per_ns = resolver.resolve_path(&path);

        let name = path.segments.last().map(|s| s.name);

        if let Some(name) = name {
            if let Some((id, vis)) = per_ns.types {
                def_map.modules[parent_module].scope.declare(name, id, vis, node_span(node), Namespace::Types);
            }
            if let Some((id, vis)) = per_ns.values {
                def_map.modules[parent_module].scope.declare(name, id, vis, node_span(node), Namespace::Values);
            }
            if let Some((id, vis)) = per_ns.macros {
                def_map.modules[parent_module].scope.declare(name, id, vis, node_span(node), Namespace::Macros);
            }
        }
    }
}

/// Import all public items from source module to target module.
fn import_all_public(source: ModuleId, target: ModuleId, def_map: &mut CrateDefMap) {
    eprintln!("[DEBUG] import_all_public called");
    let source_scope = def_map.modules[source].scope.clone();

    for (name, id, vis, span) in source_scope.types {
        if vis == Visibility::Public {
            def_map.modules[target].scope.declare(name, id, vis, span, Namespace::Types);
        }
    }
    for (name, id, vis, span) in source_scope.values {
        if vis == Visibility::Public {
            def_map.modules[target].scope.declare(name, id, vis, span, Namespace::Values);
        }
    }
}

/// Helper to guess namespace from an ID in a specific scope.
fn determine_namespace(id: LocalDefId, scope: &ItemScope) -> Namespace {
    if scope.types.iter().any(|(_, i, _, _)| *i == id) {
        return Namespace::Types;
    }
    if scope.values.iter().any(|(_, i, _, _)| *i == id) {
        return Namespace::Values;
    }
    if scope.macros.iter().any(|(_, i, _, _)| *i == id) {
        return Namespace::Macros;
    }
    Namespace::Types
}

/// Collect items from a syntax node (SourceFile or Module node) into the given module.
fn collect_items(
    node: &SyntaxNode,
    parent_module: ModuleId,
    def_map: &mut CrateDefMap,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    def_counter: &mut u32,
) {
    eprintln!("[DEBUG] collect_items called");
    for child in node.children() {
        match child.kind() {
            // Inline module: `mod name { ... }`
            SyntaxKind::Module => {
                let name_str = extract_module_name(&child);
                let name = def_map.interner.intern(&name_str);
                let span = node_span(&child);

                let is_dup = def_map.modules[parent_module]
                    .children
                    .iter()
                    .any(|(n, _)| *n == name);

                if is_dup {
                    diagnostics.push(GlyimDiagnostic::parse_error(
                        span,
                        format!("duplicate module `{}`", def_map.interner.resolve(name)),
                    ));
                } else {
                    let child_module = def_map.modules.push(ModuleData {
                        parent: Some(parent_module),
                        children: Vec::new(),
                        scope: ItemScope::default(),
                        origin: ModuleOrigin::Inline { span },
                        span,
                    });
                    def_map.modules[parent_module].children.push((name, child_module));

                    // Recurse into the module node itself
                    collect_items(
                        &child,
                        child_module,
                        def_map,
                        diagnostics,
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
                    let name = def_map.interner.intern(&name_str);
                    let vis = visibility_of_node(&child);
                    let id = LocalDefId::from_raw(*def_counter);
                    *def_counter += 1;
                    let span = node_span(&child);

                    let scope = &mut def_map.modules[parent_module].scope;
                    let existing = match ns {
                        Namespace::Types => scope.types.iter().any(|(n, _, _, _)| *n == name),
                        Namespace::Values => scope.values.iter().any(|(n, _, _, _)| *n == name),
                        Namespace::Macros => scope.macros.iter().any(|(n, _, _, _)| *n == name),
                    };
                    if existing {
                        diagnostics.push(GlyimDiagnostic::parse_error(
                            span,
                            format!("duplicate definition of `{}`", def_map.interner.resolve(name)),
                        ));
                    } else {
                        scope.declare(name, id, vis, span, ns);
                    }
                }
            }

            // Use declarations
            SyntaxKind::UseDecl => {
                process_use_decl(&child, parent_module, def_map, diagnostics);
            }

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
