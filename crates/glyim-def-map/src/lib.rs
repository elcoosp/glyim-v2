//! Module graph and lightweight definition map.
//!
//! [F6] `PathKind::Super(n)` is destructured directly.
//! [F12] Uses `glyim_core::path::{Path, PathSegment, PathKind}`.

use glyim_core::arena::{IndexVec, IdxLike};
use glyim_core::def_id::{CrateId, DefId, LocalDefId, AdtId, FnDefId, TraitDefId, ImplDefId};
use glyim_core::primitives::Visibility;
use glyim_core::interner::Name;
use glyim_core::path::{Path, PathSegment, PathKind};
use glyim_span::{Span, FileId};
use glyim_syntax::SyntaxNode;
use glyim_diag::GlyimDiagnostic;

glyim_core::define_idx!(ModuleId);

#[derive(Clone, Debug)]
pub struct CrateDefMap {
    pub root: ModuleId,
    pub modules: IndexVec<ModuleId, ModuleData>,
    pub krate: CrateId,
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
        self.types.iter().chain(self.values.iter())
            .find(|(n, _, _, _)| *n == name)
            .map(|(_, id, vis, _)| (*id, *vis))
    }

    pub fn declare(&mut self, name: Name, id: LocalDefId, vis: Visibility, span: Span, ns: Namespace) {
        let entry = (name, id, vis, span);
        match ns {
            Namespace::Types => self.types.push(entry),
            Namespace::Values => self.values.push(entry),
            Namespace::Macros => self.macros.push(entry),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Namespace { Types, Values, Macros }

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
        Self { types: Some((id, vis)), values: None, macros: None }
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
                    } else { break; }
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
                return module_data.resolve(segment.name)
                    .map(|(id, vis)| PerNs::from_types(id, vis))
                    .unwrap_or_default();
            } else {
                if let Some((_, child_id)) = module_data.children.iter().find(|(n, _)| *n == segment.name) {
                    current_module = *child_id;
                } else {
                    return PerNs::default();
                }
            }
        }
        PerNs::default()
    }

    pub fn def_map(&self) -> &CrateDefMap { self.def_map }
    pub fn module(&self) -> ModuleId { self.module }
}

#[tracing::instrument(skip(root))]
pub fn build_def_map(root: &SyntaxNode, krate: CrateId) -> (CrateDefMap, Vec<GlyimDiagnostic>) {
    let mut diagnostics = Vec::new();
    let mut modules: IndexVec<ModuleId, ModuleData> = IndexVec::new();

    let root_module = modules.push(ModuleData {
        parent: None,
        children: Vec::new(),
        scope: ItemScope::default(),
        origin: ModuleOrigin::CrateRoot,
        span: Span::DUMMY,
    });

    // Stub: collect items from CST
    for child in root.children() {
        collect_item(&child, root_module, &mut modules, &mut diagnostics);
    }

    let def_map = CrateDefMap { root: root_module, modules, krate };
    (def_map, diagnostics)
}

fn collect_item(node: &SyntaxNode, module: ModuleId, _modules: &mut IndexVec<ModuleId, ModuleData>, _diagnostics: &mut Vec<GlyimDiagnostic>) {
    use glyim_syntax::SyntaxKind::*;
    match node.kind() {
        FnDef | StructDef | EnumDef | TraitDef | ImplDef | TypeAlias
        | ConstDef | StaticDef | UseDecl | ExternBlock | Module => {
            // STUB: real implementation extracts name, visibility, etc.
        }
        _ => {}
    }
}
