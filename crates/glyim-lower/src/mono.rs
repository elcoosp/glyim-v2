//! Monomorphization: instantiate generic MIR bodies with concrete types.
use glyim_core::arena::IndexVec;
use glyim_core::def_id::{ConstDefId, CrateId, DefId, FnDefId, LocalDefId, StaticDefId};
use glyim_mir::{self, MirConstKind, Operand, Rvalue, StatementKind, TerminatorKind};
use glyim_type::*;
use std::sync::Arc;

glyim_core::define_idx!(MonoItemId);

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
    DropGlue {
        ty: Ty,
    },
}

#[derive(Clone, Debug)]
pub struct MonoItemData {
    pub item: MonoItem,
    pub body: Arc<glyim_mir::Body>,
    pub symbol: String,
    pub source_module: u32,
}

pub struct MonoCtx {
    items: IndexVec<MonoItemId, MonoItemData>,
    queue: std::collections::VecDeque<MonoItem>,
    seen: std::collections::HashSet<MonoItem>,
    cache: std::collections::HashMap<MonoItem, MonoItemId>,
    drop_locals: Vec<glyim_mir::LocalIdx>,
}

impl MonoCtx {
    pub fn new() -> Self {
        Self {
            items: IndexVec::new(),
            queue: std::collections::VecDeque::new(),
            seen: std::collections::HashSet::new(),
            cache: std::collections::HashMap::new(),
            drop_locals: Vec::new(),
        }
    }

    fn enqueue(&mut self, item: MonoItem) {
        if !self.seen.contains(&item) && !self.cache.contains_key(&item) {
            self.queue.push_back(item);
        }
    }

    pub fn collect(
        &mut self,
        start: &[MonoItem],
        mir_bodies: &dyn Fn(DefId, &Substitution) -> Arc<glyim_mir::Body>,
        drop_glue_body: &dyn Fn(Ty) -> Arc<glyim_mir::Body>,
    ) {
        for item in start {
            if self.cache.contains_key(item) || self.seen.contains(item) {
                continue;
            }
            self.queue.push_back(item.clone());
        }

        while let Some(item) = self.queue.pop_front() {
            if self.seen.contains(&item) || self.cache.contains_key(&item) {
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
                MonoItem::DropGlue { ty } => drop_glue_body(*ty),
            };

            self.scan_body_for_refs(&body);
            let symbol = format!("{:?}", item);
            let id = self.items.push(MonoItemData {
                item: item.clone(),
                body,
                symbol,
                source_module: 0,
            });
            self.cache.insert(item, id);
        }
    }

    fn scan_body_for_refs(&mut self, body: &glyim_mir::Body) {
        self.drop_locals.clear();

        for block in body.basic_blocks.iter() {
            for stmt in &block.statements {
                if let StatementKind::Assign(_, ref rvalue) = stmt.kind {
                    self.scan_rvalue(rvalue);
                }
            }
            self.scan_terminator(&block.terminator.kind);
        }

        let pending_drops: Vec<_> = self.drop_locals.drain(..).collect();
        for local_idx in pending_drops {
            let local_raw = local_idx.to_raw() as usize;
            if let Some(local_decl) = body
                .locals
                .get(glyim_mir::LocalIdx::from_raw(local_raw as u32))
            {
                let drop_ty = local_decl.ty;
                self.enqueue(MonoItem::DropGlue { ty: drop_ty });
            }
        }
    }

    fn scan_rvalue(&mut self, rvalue: &Rvalue) {
        match rvalue {
            Rvalue::Use(operand) => self.scan_operand(operand),
            Rvalue::BinaryOp(_, operands) => {
                let (lhs, rhs) = operands.as_ref();
                self.scan_operand(lhs);
                self.scan_operand(rhs);
            }
            Rvalue::UnaryOp(_, operand) => self.scan_operand(operand),
            Rvalue::Aggregate(_, operands) => {
                for operand in operands {
                    self.scan_operand(operand);
                }
            }
            Rvalue::Cast(_, operand, _) => self.scan_operand(operand),
            Rvalue::Repeat(operand, _) => self.scan_operand(operand),
            _ => {}
        }
    }

    fn scan_operand(&mut self, operand: &Operand) {
        if let Operand::Constant(mir_const) = operand {
            self.scan_const(mir_const);
        }
    }

    fn scan_const(&mut self, mir_const: &glyim_mir::MirConst) {
        match &mir_const.kind {
            MirConstKind::Fn(def_id, substs) => {
                self.enqueue(MonoItem::Fn {
                    def_id: *def_id,
                    substs: *substs,
                });
            }
            // ConstRef constants are materialized by the backend as zero-
            // initialized globals (`__glyim_const_{id}`); full const
            // evaluation/lowering is a follow-up, so we do not enqueue a
            // `MonoItem::Const` (which would require a lowered const body)
            // for value-namespace const references yet.
            MirConstKind::ConstRef(_, _) => {}
            _ => {}
        }
    }

    fn scan_terminator(&mut self, kind: &TerminatorKind) {
        match kind {
            TerminatorKind::Call { func, args, .. } => {
                self.scan_operand(func);
                for arg in args {
                    self.scan_operand(arg);
                }
            }
            TerminatorKind::Drop { place, .. } => {
                self.drop_locals.push(place.local);
            }
            TerminatorKind::SwitchInt { discr, .. } => self.scan_operand(discr),
            TerminatorKind::Assert { cond, .. } => self.scan_operand(cond),
            _ => {}
        }
    }

    pub fn items(&self) -> &[MonoItemData] {
        self.items.as_slice()
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    pub fn lookup(&self, item: &MonoItem) -> Option<MonoItemId> {
        self.cache.get(item).copied()
    }

    /// Apply polymorphization to all collected mono items and deduplicate identical instantiations.
    ///
    /// This should be called after `collect` finishes and before codegen. It uses the
    /// `polymorphize` module (already present in `glyim-lower`) to analyse which generic
    /// parameters are actually used by each MIR body, shrink substitutions accordingly, and
    /// merge duplicate items so that codegen only sees unique instantiations.
    pub fn polymorphize_and_deduplicate(&mut self, ctx: &mut TyCtxMut) {
        if self.items.is_empty() {
            return;
        }
        let deduped = crate::polymorphize::deduplicate(ctx, self.items.as_slice());
        self.items = IndexVec::new();
        self.cache.clear();
        for data in deduped {
            let item = data.item.clone();
            let id = self.items.push(data);
            self.cache.insert(item, id);
        }
    }
}

impl Default for MonoCtx {
    fn default() -> Self {
        Self::new()
    }
}
