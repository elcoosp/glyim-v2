use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, LocalDefId};
use glyim_core::interner::Interner;
use glyim_core::primitives::Visibility;
use glyim_def_map::{CrateDefMap, ItemScope, ModuleData, ModuleId, ModuleOrigin};
use glyim_span::Span;
use glyim_type::TyCtxMut;

pub fn empty_def_map() -> CrateDefMap {
    let mut modules: IndexVec<ModuleId, ModuleData> = IndexVec::new();
    let root = modules.push(ModuleData {
        parent: None,
        children: vec![],
        scope: ItemScope {
            types: vec![],
            values: vec![],
            macros: vec![],
        },
        origin: ModuleOrigin::CrateRoot,
        span: Span::DUMMY,
        def_id: LocalDefId::from_raw(0),
        visibility: Visibility::Public,
    });
    CrateDefMap {
        root,
        modules,
        krate: CrateId::from_raw(0),
        interner: Interner::new(),
    }
}
pub fn make_ty_ctx() -> TyCtxMut {
    TyCtxMut::new(Interner::new())
