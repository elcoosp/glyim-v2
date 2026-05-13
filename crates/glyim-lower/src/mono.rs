//! Monomorphization: instantiate generic MIR bodies with concrete types.

use glyim_core::arena::IndexVec;
use glyim_core::def_id::{ConstDefId, CrateId, DefId, FnDefId, LocalDefId, StaticDefId};
use glyim_mir;
use glyim_type::*;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MonoItem {
    Fn {
        def_id: FnDefId,
        substs: Substitution,
    },
    Const {
        def_id: ConstDefId,
        substs: Substitution,
    },
    Static {
        def_id: StaticDefId,
    },
}

glyim_core::define_idx!(MonoItemId);

#[derive(Clone, Debug)]
pub struct MonoItemData {
    pub item: MonoItem,
    pub body: Arc<glyim_mir::Body>,
    pub symbol: String,
}

pub struct MonoCtx {
    items: IndexVec<MonoItemId, MonoItemData>,
    queue: std::collections::VecDeque<MonoItem>,
    seen: std::collections::HashSet<MonoItem>,
}

impl MonoCtx {
    pub fn new() -> Self {
        Self {
            items: IndexVec::new(),
            queue: std::collections::VecDeque::new(),
            seen: std::collections::HashSet::new(),
        }
    }

    #[tracing::instrument(level = "info", skip(self, mir_bodies))]
    pub fn collect(
        &mut self,
        start: &[MonoItem],
        mir_bodies: &dyn Fn(DefId, &Substitution) -> Arc<glyim_mir::Body>,
    ) {
        self.queue.extend(start.iter().cloned());
        while let Some(item) = self.queue.pop_front() {
            if self.seen.contains(&item) {
                continue;
            }
            self.seen.insert(item.clone());
            let body = match &item {
                MonoItem::Fn { def_id, substs } => mir_bodies(
                    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(def_id.to_raw())),
                    substs,
                ),
                MonoItem::Const { def_id, substs } => mir_bodies(
                    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(def_id.to_raw())),
                    substs,
                ),
                MonoItem::Static { .. } => Arc::new(glyim_mir::Body::dummy(DefId::new(
                    CrateId::from_raw(0),
                    LocalDefId::from_raw(0),
                ))),
            };
            let symbol = format!("{:?}", item);
            self.items.push(MonoItemData {
                item: item.clone(),
                body,
                symbol,
            });
        }
    }

    pub fn instantiate(
        ctx: &TyCtx,
        body: &glyim_mir::Body,
        substs: &Substitution,
    ) -> glyim_mir::Body {
        if substs.is_empty() {
            return body.clone();
        }
        let _ = (ctx, substs);
        body.clone() // STUB
    }

    pub fn items(&self) -> &[MonoItemData] {
        self.items.as_slice()
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

impl Default for MonoCtx {
    fn default() -> Self {
        Self::new()
    }
}
