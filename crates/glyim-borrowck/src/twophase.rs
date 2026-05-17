//! Two‑phase borrow activation analysis – same block only.
//!
//! A two‑phase mutable borrow starts in reservation phase and becomes
//! activated when the borrowed reference is first read. Activation can
//! only happen within the same basic block where the borrow was created.
//! If a two‑phase borrow crosses a block boundary, it is considered
//! already activated (conservative, matches Rust's semantics).

use fixedbitset::FixedBitSet as BitSet;
use glyim_mir::{BasicBlockIdx, Body, LocalIdx, StatementKind, BorrowKind, Rvalue, Operand, Place};
use crate::visitor::{LocalReadChecker, walk_rvalue_reads};

/// Result of the reservation analysis for a single loan.
pub struct ReservationAnalysis {
    per_block: Vec<BitSet>,
    creation_block: BasicBlockIdx,
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
        let stmt_counts: Vec<usize> = body.basic_blocks
            .iter()
            .map(|b| b.statements.len())
            .collect();

        let mut per_block: Vec<BitSet> = stmt_counts
            .iter()
            .map(|&len| BitSet::with_capacity(len + 1))
            .collect();

        let block_data = &body.basic_blocks[loan_block];
        let num_stmts = block_data.statements.len();
        let reservation = &mut per_block[loan_block.to_raw() as usize];

        let start_point = if loan_stmt + 1 < num_stmts {
            loan_stmt + 1
        } else {
            num_stmts
        };
        reservation.insert(start_point);

        for point in start_point..=num_stmts {
            if point == num_stmts {
                break;
            }
            let stmt = &block_data.statements[point];
            let mut checker = LocalReadChecker::new(dest_local);
            if let StatementKind::Assign(_, rvalue) = &stmt.kind {
                walk_rvalue_reads(rvalue, &mut checker);
            }
            if checker.found() {
                break;
            }
            let next = point + 1;
            if next <= num_stmts {
                reservation.insert(next);
            }
        }

        ReservationAnalysis { per_block, creation_block: loan_block }
    }

    pub fn is_reservation(&self, block: BasicBlockIdx, stmt_idx: usize) -> bool {
        if block != self.creation_block {
            return false;
        }
        self.per_block
            .get(block.to_raw() as usize)
            .map(|bits| bits.contains(stmt_idx))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyim_mir::{
        BasicBlockData, Body, LocalDecl, LocalIdx, Statement, StatementKind,
        Terminator, TerminatorKind, SourceInfo, BorrowKind, Operand, Place,
    };
    use glyim_core::def_id::{DefId, CrateId, LocalDefId};
    use glyim_core::primitives::Mutability;
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
                    BorrowKind::Mut { allow_two_phase_borrow: true },
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
    fn test_cross_block_returns_false() {
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
                Rvalue::Ref(Place::new(local_1), BorrowKind::Mut { allow_two_phase_borrow: true }),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        };
        let block0 = BasicBlockData {
            statements: vec![borrow_stmt],
            terminator: Terminator { kind: TerminatorKind::Goto { target: BasicBlockIdx::from_raw(1) }, source_info: SourceInfo::new(Span::DUMMY) },
            is_cleanup: false,
        };
        let block1 = BasicBlockData {
            statements: vec![],
            terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
            is_cleanup: false,
        };
        body.basic_blocks.push(block0);
        body.basic_blocks.push(block1);
        let analysis = ReservationAnalysis::compute(&body, BasicBlockIdx::from_raw(0), 0, local_2);
        assert!(!analysis.is_reservation(BasicBlockIdx::from_raw(1), 0));
    }
}
