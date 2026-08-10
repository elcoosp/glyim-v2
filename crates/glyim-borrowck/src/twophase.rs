//! Two‑phase borrow activation analysis – same block only.
//!
//! A two‑phase mutable borrow starts in reservation phase and becomes
//! activated when the borrowed reference is first read. Activation can
//! only happen within the same basic block where the borrow was created.
//! If a two‑phase borrow crosses a block boundary, it is considered
//! already activated (conservative, matches Rust's semantics).

use crate::visitor::{LocalReadChecker, walk_rvalue_reads};
use fixedbitset::FixedBitSet as BitSet;
use glyim_mir::{BasicBlockIdx, Body, LocalIdx, StatementKind};

/// Result of the reservation analysis for a single loan.
pub struct ReservationAnalysis {
    per_block: Vec<BitSet>,
}

impl ReservationAnalysis {
    /// Compute the reservation points for a two‑phase loan created at
    /// `(loan_block, loan_stmt)` with destination local `dest_local`.
    pub fn compute(
        body: &Body,
        loan_block: BasicBlockIdx,
        loan_stmt: usize,
        dest_local: LocalIdx,
    ) -> Self {
        let stmt_counts: Vec<usize> = body
            .basic_blocks
            .iter()
            .map(|b| b.statements.len())
            .collect();
        let mut per_block: Vec<BitSet> = stmt_counts
            .iter()
            .map(|&len| BitSet::with_capacity(len + 1))
            .collect();
        use std::collections::{VecDeque, HashSet};
        let mut worklist: VecDeque<(BasicBlockIdx, usize)> = VecDeque::new();
        let mut visited: HashSet<BasicBlockIdx> = HashSet::new();
        let block_data = &body.basic_blocks[loan_block];
        let num_stmts = block_data.statements.len();
        let start_point = if loan_stmt + 1 < num_stmts {
            loan_stmt + 1
        } else {
            num_stmts
        };
        per_block[loan_block.to_raw() as usize].insert(start_point);
        if start_point < num_stmts {
            worklist.push_back((loan_block, start_point));
        } else {
            for succ in crate::visitor::successor_blocks(&block_data.terminator.kind) {
                worklist.push_back((succ, 0));
            }
        }
        while let Some((block, start_stmt)) = worklist.pop_front() {
            if !visited.insert(block) {
                continue;
            }
            let block_data = &body.basic_blocks[block];
            let num_stmts = block_data.statements.len();
            let reservation = &mut per_block[block.to_raw() as usize];
            reservation.insert(start_stmt);
            let mut activated = false;
            for point in start_stmt..num_stmts {
                let stmt = &block_data.statements[point];
                let mut checker = LocalReadChecker::new(dest_local);
                if let StatementKind::Assign(_, rvalue) = &stmt.kind {
                    walk_rvalue_reads(rvalue, &mut checker);
                }
                if checker.found() {
                    activated = true;
                    break;
                }
                let next = point + 1;
                if next <= num_stmts {
                    reservation.insert(next);
                }
            }
            if !activated {
                for succ in crate::visitor::successor_blocks(&block_data.terminator.kind) {
                    worklist.push_back((succ, 0));
                }
            }
        }
        ReservationAnalysis { per_block }
    }

    pub fn is_reservation(&self, block: BasicBlockIdx, stmt_idx: usize) -> bool {
        self.per_block
            .get(block.to_raw() as usize)
            .map(|bits| bits.contains(stmt_idx))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyim_core::def_id::{CrateId, DefId, LocalDefId};
    use glyim_core::primitives::Mutability;
    use glyim_mir::{
        BasicBlockData, Body, BorrowKind, LocalDecl, Place, Rvalue, SourceInfo, Statement,
        StatementKind, Terminator, TerminatorKind,
    };
    use glyim_span::Span;
    use glyim_type::Ty;

    #[test]
    fn test_same_block_no_activation() {
        // Build a fresh body from scratch – no dummy block.
        let mut body = Body {
            owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
            basic_blocks: glyim_core::arena::IndexVec::new(),
            locals: glyim_core::arena::IndexVec::new(),
            arg_count: 0,
            return_ty: Ty::UNIT,
            span: Span::DUMMY,
            var_debug_info: Vec::new(),
        };

        // Add local declarations.
        let local_1 = body.locals.push(LocalDecl {
            ty: Ty::BOOL,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });
        let local_2 = body.locals.push(LocalDecl {
            ty: Ty::ERROR, // placeholder for reference type
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        // Create a borrow statement.
        let borrow_stmt = Statement {
            kind: StatementKind::Assign(
                Place::new(local_2),
                Rvalue::Ref(
                    Place::new(local_1),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: true,
                    },
                ),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        };
        let block = BasicBlockData {
            statements: vec![borrow_stmt],
            terminator: Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        };
        body.basic_blocks.push(block);

        let analysis = ReservationAnalysis::compute(&body, BasicBlockIdx::from_raw(0), 0, local_2);
        assert!(analysis.is_reservation(BasicBlockIdx::from_raw(0), 1));
        assert!(!analysis.is_reservation(BasicBlockIdx::from_raw(0), 0));
    }

    #[test]
    fn test_cross_block_extends_if_not_activated() {
        let mut body = Body {
            owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
            basic_blocks: glyim_core::arena::IndexVec::new(),
            locals: glyim_core::arena::IndexVec::new(),
            arg_count: 0,
            return_ty: Ty::UNIT,
            span: Span::DUMMY,
            var_debug_info: Vec::new(),
        };
        let local_1 = body.locals.push(LocalDecl {
            ty: Ty::BOOL,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });
        let local_2 = body.locals.push(LocalDecl {
            ty: Ty::ERROR,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });
        let borrow_stmt = Statement {
            kind: StatementKind::Assign(
                Place::new(local_2),
                Rvalue::Ref(
                    Place::new(local_1),
                    BorrowKind::Mut {
                        allow_two_phase_borrow: true,
                    },
                ),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        };
        let block0 = BasicBlockData {
            statements: vec![borrow_stmt],
            terminator: Terminator {
                kind: TerminatorKind::Goto {
                    target: BasicBlockIdx::from_raw(1),
                },
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        };
        let block1 = BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        };
        body.basic_blocks.push(block0);
        body.basic_blocks.push(block1);
        let analysis = ReservationAnalysis::compute(&body, BasicBlockIdx::from_raw(0), 0, local_2);
        assert!(analysis.is_reservation(BasicBlockIdx::from_raw(1), 0));
    }
}
