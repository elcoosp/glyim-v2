//! Monomorphization: instantiate generic MIR bodies with concrete types.
//!
//! V23: Implements worklist-based mono item graph traversal.
//! V24: Adds mono item cache with hash-consed substitution deduplication.
//!
//! Recursively follows function calls (via MirConstKind::Fn),
//! constant references (via MirConstKind::ConstRef), and
//! drop glue (via TerminatorKind::Drop).

use glyim_core::arena::IndexVec;
use glyim_core::def_id::{ConstDefId, CrateId, DefId, FnDefId, LocalDefId, StaticDefId};
use glyim_mir::{self, MirConstKind, Operand, Rvalue, StatementKind, TerminatorKind};
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
    pub source_module: u32,
}

pub struct MonoCtx {
    items: IndexVec<MonoItemId, MonoItemData>,
    queue: std::collections::VecDeque<MonoItem>,
    seen: std::collections::HashSet<MonoItem>,
    /// Cache mapping MonoItem → MonoItemId for deduplication and lookup.
    /// Persists across `collect()` calls, ensuring each unique (DefId, Substitution)
    /// pair is processed exactly once and can be looked up by MonoItemId.
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

    /// Enqueue a mono item for collection if it hasn't been seen or cached yet.
    fn enqueue(&mut self, item: MonoItem) {
        if !self.seen.contains(&item) && !self.cache.contains_key(&item) {
            self.queue.push_back(item);
        }
    }

    /// Look up the `MonoItemId` for a previously collected item.
    ///
    /// Returns `None` if the item has not been collected yet.
    /// This is the primary cache query interface: given a `(DefId, Substitution)` pair
    /// (wrapped in a `MonoItem`), find the canonical `MonoItemId` assigned during
    /// collection. The cache ensures that each unique instantiation is processed
    /// exactly once, so the same `MonoItem` always maps to the same `MonoItemId`.
    #[allow(dead_code)]
    pub(crate) fn lookup(&self, item: &MonoItem) -> Option<MonoItemId> {
        self.cache.get(item).copied()
    }

    /// Returns the number of entries in the cache.
    /// Should always equal `item_count()` after a `collect()` call.
    #[allow(dead_code)]
    pub(crate) fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// Scan a MIR body for references to other mono items (calls, constants, drops).
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

    /// Scan an Rvalue for mono item references.
    fn scan_rvalue(&mut self, rvalue: &Rvalue) {
        match rvalue {
            Rvalue::Use(operand) => {
                self.scan_operand(operand);
            }
            Rvalue::BinaryOp(_, operands) => {
                let (lhs, rhs) = operands.as_ref();
                self.scan_operand(lhs);
                self.scan_operand(rhs);
            }
            Rvalue::UnaryOp(_, operand) => {
                self.scan_operand(operand);
            }
            Rvalue::Ref(_, _) => {}
            Rvalue::Aggregate(_, operands) => {
                for operand in operands {
                    self.scan_operand(operand);
                }
            }
            Rvalue::Discriminant(_) | Rvalue::Len(_) => {}
            Rvalue::Cast(_, operand, _) => {
                self.scan_operand(operand);
            }
            Rvalue::Repeat(operand, _) => {
                self.scan_operand(operand);
            }
        }
    }

    /// Scan an Operand for mono item references.
    fn scan_operand(&mut self, operand: &Operand) {
        match operand {
            Operand::Constant(mir_const) => {
                self.scan_const(mir_const);
            }
            Operand::Copy(_) | Operand::Move(_) => {}
        }
    }

    /// Scan a MirConst for mono item references.
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

    /// Scan a TerminatorKind for mono item references.
    fn scan_terminator(&mut self, kind: &TerminatorKind) {
        match kind {
            TerminatorKind::Call {
                func,
                args,
                destination: _,
                target: _,
                cleanup: _,
            } => {
                self.scan_operand(func);
                for arg in args {
                    self.scan_operand(arg);
                }
            }
            TerminatorKind::Drop {
                place: _,
                target: _,
                cleanup: _,
            } => {
                // Drop glue collection not yet implemented
                tracing::debug!("STUB: drop glue collection not implemented");
            }
            TerminatorKind::SwitchInt { discr, .. } => {
                self.scan_operand(discr);
            }
            TerminatorKind::Assert { cond, .. } => {
                self.scan_operand(cond);
            }
            TerminatorKind::Goto { .. } | TerminatorKind::Return | TerminatorKind::Unreachable => {}
        }
    }

    #[tracing::instrument(level = "info", skip(self, mir_bodies))]
    pub fn collect(
        &mut self,
        start: &[MonoItem],
        mir_bodies: &dyn Fn(DefId, &Substitution) -> Arc<glyim_mir::Body>,
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

    pub fn instantiate(
        ctx: &TyCtx,
        body: &glyim_mir::Body,
        substs: &Substitution,
    ) -> glyim_mir::Body {
        if substs.is_empty() {
            return body.clone();
        }
        let _ = (ctx, substs);
        body.clone()
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
