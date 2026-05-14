use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, LocalDefId};
use glyim_core::interner::{Interner, Name};
use glyim_core::primitives::*;
use glyim_def_map::{CrateDefMap, ItemScope, ModuleData, ModuleId, ModuleOrigin};
use glyim_hir::{Body, BodyId, CrateHir, Expr, ExprId, FnItem, Item, ItemId, ItemKind, Pat, PatId};
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
}

pub fn make_name(inter: &mut Interner, s: &str) -> Name {
    inter.intern(s)
}

pub fn build_hir_one_fn(
    name: Name,
    params: Vec<(Name, Pat)>,
    body_exprs: Vec<Expr>,
) -> (Interner, CrateHir) {
    let inter = Interner::new();
    let _ = (name, params, body_exprs); // TODO
    (inter, CrateHir {
        items: IndexVec::new(),
        bodies: IndexVec::new(),
        body_owners: IndexVec::new(),
    })
}
