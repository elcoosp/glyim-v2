use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::interner::Name;
use glyim_span::Span;

glyim_core::define_idx!(ExprId);
glyim_core::define_idx!(PatId);
glyim_core::define_idx!(BodyId);
glyim_core::define_idx!(ItemId);

#[derive(Clone, Debug)] pub struct CrateHir {
    pub items: IndexVec<ItemId, Item>,
    pub bodies: IndexVec<BodyId, Body>,
    pub body_owners: IndexVec<BodyId, LocalDefId>,
}
#[derive(Clone, Debug)] pub struct Item { pub name: Name, pub kind: ItemKind, pub span: Span }
#[derive(Clone, Debug)] pub enum ItemKind { Fn(FnItem) }
#[derive(Clone, Debug)] pub struct FnItem { pub body: Option<BodyId> }
#[derive(Clone, Debug)] pub struct Body { pub owner: LocalDefId }
