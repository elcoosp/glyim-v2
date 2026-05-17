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
    }, // New variant for drop glue
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
}

impl MonoCtx {
    pub fn new() -> Self {
        Self {
            items: IndexVec::new(),
            queue: std::collections::VecDeque::new(),
            seen: std::collections::HashSet::new(),
            cache: std::collections::HashMap::new(),
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
        for block in body.basic_blocks.iter() {
            for stmt in &block.statements {
                if let StatementKind::Assign(_, ref rvalue) = stmt.kind {
                    self.scan_rvalue(rvalue);
                }
            }
            self.scan_terminator(&block.terminator.kind);
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
            MirConstKind::ConstRef(def_id, substs) => {
                self.enqueue(MonoItem::Const {
                    def_id: *def_id,
                    substs: *substs,
                });
            }
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
                // In a real implementation we would get the type of the place from the body's local decls.
                // Since we don't have access to the body here, we need to pass the type.
                // This is a placeholder; the actual monomorphization collector will need to have
                // access to the body's locals to compute the type.
                // For now, we'll skip and rely on the caller to handle drop glue generation.
                // In the full implementation, this method would be called with access to the body.
                // We'll add a stub warning.
                tracing::warn!("STUB: drop glue scanning not fully implemented");
            }
            TerminatorKind::SwitchInt { discr, .. } => self.scan_operand(discr),
            TerminatorKind::Assert { cond, .. } => self.scan_operand(cond),
            _ => {}
        }
    }

    pub fn items(&self) -> &[MonoItemData] {
        self.items.as_slice()
    }

    /// Returns the number of collected mono items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns the number of entries in the cache (should equal item_count).
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// Look up a mono item in the cache, returning its MonoItemId if found.
    pub fn lookup(&self, item: &MonoItem) -> Option<MonoItemId> {
        self.cache.get(item).copied()
    }
}

impl Default for MonoCtx {
    fn default() -> Self {
        Self::new()
    }
}
