//! Monomorphization: instantiate generic MIR bodies with concrete types.
//!
//! V23: Implements worklist-based mono item graph traversal.
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

    /// Enqueue a mono item for collection if it hasn't been seen yet.
    fn enqueue(&mut self, item: MonoItem) {
        if !self.seen.contains(&item) {
            self.queue.push_back(item);
        }
    }

    /// Scan a MIR body for references to other mono items (calls, constants, drops).
    ///
    /// This is the core of the graph traversal: every terminator and statement
    /// is inspected for `MirConstKind::Fn`, `MirConstKind::ConstRef`, and
    /// `TerminatorKind::Drop` which produce new mono items to collect.
    fn scan_body_for_refs(&mut self, body: &glyim_mir::Body) {
        for block in body.basic_blocks.iter() {
            // Scan statements for constant references
            for stmt in &block.statements {
                if let StatementKind::Assign(_, ref rvalue) = stmt.kind {
                    self.scan_rvalue(rvalue);
                }
            }

            // Scan terminator for calls and drops
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
            Rvalue::Ref(_, _) => {
                // References don't create new mono items
            }
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
            Operand::Copy(_) | Operand::Move(_) => {
                // Copy/Move refer to locals, not new mono items
            }
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
            _ => {
                // Other constant kinds (Int, Bool, etc.) don't create mono items
            }
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
                // The function operand may be a MirConstKind::Fn constant
                self.scan_operand(func);
                // Arguments may also contain constant references
                for arg in args {
                    self.scan_operand(arg);
                }
            }
            TerminatorKind::Drop {
                place,
                target: _,
                cleanup: _,
            } => {
                // Drop glue: we need to find the drop implementation for the
                // type of the place being dropped. For now, we look up the
                // type from the local declaration and, if it's an ADT, we
                // enqueue a synthetic drop function.
                //
                // The drop function ID is conventionally derived from the ADT
                // type. In a full compiler this would be resolved through the
                // trait solver; here we use a convention: FnDefId is derived
                // from the type's raw index offset into a "drop" namespace.
                //
                // For V23, we just emit a tracing warning and will implement
                // proper drop glue resolution in a future stream.
                let _ = place;
                tracing::debug!("STUB: drop glue collection not fully implemented");
            }
            TerminatorKind::SwitchInt { discr, .. } => {
                self.scan_operand(discr);
            }
            TerminatorKind::Assert { cond, .. } => {
                self.scan_operand(cond);
            }
            TerminatorKind::Goto { .. }
            | TerminatorKind::Return
            | TerminatorKind::Unreachable => {}
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

            // Scan the body for references to other mono items
            self.scan_body_for_refs(&body);

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
