use glyim_core::arena::IndexVec;
use glyim_core::def_id::CrateId;
use glyim_span::Span;
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
    pub children: Vec<(glyim_core::interner::Name, ModuleId)>,
    pub scope: ItemScope,
    pub origin: ModuleOrigin,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ModuleOrigin {
    File { file_id: glyim_span::FileId },
    Inline { span: Span },
    CrateRoot,
}

#[derive(Clone, Debug, Default)]
pub struct ItemScope {
    pub types: Vec<(glyim_core::interner::Name, glyim_core::def_id::LocalDefId, glyim_core::primitives::Visibility, Span)>,
    pub values: Vec<(glyim_core::interner::Name, glyim_core::def_id::LocalDefId, glyim_core::primitives::Visibility, Span)>,
    pub macros: Vec<(glyim_core::interner::Name, glyim_core::def_id::LocalDefId, glyim_core::primitives::Visibility, Span)>,
}

pub fn build_def_map(root: &SyntaxNode, krate: CrateId) -> (CrateDefMap, Vec<GlyimDiagnostic>) {
    let modules = IndexVec::new();
    let root_module = ModuleId::from_raw(0);
    let def_map = CrateDefMap { root: root_module, modules, krate };
    (def_map, Vec::new())
}
