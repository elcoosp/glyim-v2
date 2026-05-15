//! Borrow checker using non-lexical lifetimes (NLL) with Polonius-style
//! region inference.
//!
//! This implementation tracks borrows across basic block boundaries using
//! a CFG-aware liveness analysis. Loans are tracked as sets, and conflicts
//! are detected between active borrows and place accesses.
//!
//! The analysis proceeds in three phases:
//! 1. **Loan collection**: Scan the MIR body for `Rvalue::Ref` assignments
//!    and record each as a `Loan` with the borrowed place, borrow kind,
//!    and the local holding the reference.
//! 2. **Liveness analysis**: Compute which locals are live at each program
//!    point using a standard backward dataflow analysis on the CFG.
//! 3. **Conflict detection**: For each statement, determine which loans are
//!    active (their dest local is live) and check for conflicts with
//!    place accesses in the statement.

use fixedbitset::FixedBitSet as BitSet;
use glyim_diag::{DiagSeverity, GlyimDiagnostic, MultiSpan, SubDiagnostic};
use glyim_mir::{
    BasicBlockIdx, Body, BorrowKind, LocalIdx, Operand, Rvalue, StatementKind, TerminatorKind,
};
use glyim_span::Span;
use glyim_type::TyCtx;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct BorrowckResult {
    pub errors: Vec<GlyimDiagnostic>,
}

pub trait BorrowckCtx {
    fn ty_ctx(&self) -> &TyCtx;
    fn local_decl(&self, local: LocalIdx) -> &glyim_mir::LocalDecl;
    fn is_copy(&self, ty: glyim_type::Ty) -> bool;
}

// ---------------------------------------------------------------------------
// Loan representation
// ---------------------------------------------------------------------------

/// A loan represents a borrow of a place at a particular program point.
#[derive(Clone, Debug)]
struct Loan {
    /// The local that holds the reference (destination of the `Rvalue::Ref`).
    dest_local: LocalIdx,
    /// The root local of the borrowed place.
    borrowed_local: LocalIdx,
    /// The kind of borrow.
    kind: BorrowKind,
    /// The span for error reporting.
    span: Span,
}

/// Scan the MIR body for all `Rvalue::Ref` assignments and record loans.
fn collect_loans(body: &Body) -> Vec<Loan> {
    let mut loans = Vec::new();
    for (_block_idx, block_data) in body.basic_blocks.iter_enumerated() {
        for stmt in block_data.statements.iter() {
            if let StatementKind::Assign(dest, Rvalue::Ref(borrowed, kind)) = &stmt.kind {
                loans.push(Loan {
                    dest_local: dest.local,
                    borrowed_local: borrowed.local,
                    kind: *kind,
                    span: stmt.source_info.span,
                });
            }
        }
    }
    loans
}

// ---------------------------------------------------------------------------
// CFG liveness analysis
// ---------------------------------------------------------------------------

/// Result of the backward dataflow liveness analysis.
struct LivenessResult {
    /// For each basic block, the set of locals live on entry.
    /// Kept for debugging and future use (e.g., region inference).
    #[allow(dead_code)]
    live_in: Vec<BitSet>,
    /// For each basic block, the set of locals live on exit.
    live_out: Vec<BitSet>,
}

/// Compute liveness of all locals using a standard backward dataflow
/// analysis on the CFG.
///
/// A local is *live* at a program point if its current value may be used
/// on some execution path starting from that point before being overwritten.
fn compute_liveness(body: &Body) -> LivenessResult {
    let num_blocks = body.basic_blocks.len();
    let num_locals = body.locals.len();

    let mut live_in: Vec<BitSet> = (0..num_blocks)
        .map(|_| BitSet::with_capacity(num_locals))
        .collect();
    let mut live_out: Vec<BitSet> = (0..num_blocks)
        .map(|_| BitSet::with_capacity(num_locals))
        .collect();

    // Per-block "used before defined" and "defined" sets.
    let mut block_uses: Vec<BitSet> = (0..num_blocks)
        .map(|_| BitSet::with_capacity(num_locals))
        .collect();
    let mut block_defs: Vec<BitSet> = (0..num_blocks)
        .map(|_| BitSet::with_capacity(num_locals))
        .collect();

    for (block_idx, block_data) in body.basic_blocks.iter_enumerated() {
        let bu = &mut block_uses[block_idx.to_raw() as usize];
        let bd = &mut block_defs[block_idx.to_raw() as usize];

        for stmt in &block_data.statements {
            if let StatementKind::Assign(place, rvalue) = &stmt.kind {
                // Record the definition *before* collecting uses so that
                // a local defined and then used within the same statement
                // is not counted as "used before defined".
                bd.insert(place.local.to_raw() as usize);
                collect_rvalue_uses(rvalue, bu, bd);
            }
        }

        // Terminator uses come after all statement defs.
        collect_terminator_uses(&block_data.terminator.kind, bu, bd);
    }

    // Fixed-point iteration (backward).
    let mut changed = true;
    while changed {
        changed = false;
        for (block_idx, block_data) in body.basic_blocks.iter_enumerated() {
            let bi = block_idx.to_raw() as usize;

            // live_out(B) = ∪ live_in(S) for all successors S
            let mut new_live_out = BitSet::with_capacity(num_locals);
            for succ in successor_blocks(&block_data.terminator.kind) {
                new_live_out.union_with(&live_in[succ.to_raw() as usize]);
            }

            // live_in(B) = uses(B) ∪ (live_out(B) − defs(B))
            let mut new_live_in = block_uses[bi].clone();
            let mut diff = new_live_out.clone();
            diff.difference_with(&block_defs[bi]);
            new_live_in.union_with(&diff);

            if new_live_in != live_in[bi] || new_live_out != live_out[bi] {
                changed = true;
                live_in[bi] = new_live_in;
                live_out[bi] = new_live_out;
            }
        }
    }

    LivenessResult { live_in, live_out }
}

/// Record locals that are *used* (read) by an rvalue, but only if they
/// have not already been defined earlier in the same block.
fn collect_rvalue_uses(rvalue: &Rvalue, uses: &mut BitSet, defs: &BitSet) {
    match rvalue {
        Rvalue::Use(operand) => collect_operand_uses(operand, uses, defs),
        Rvalue::Ref(place, _) => {
            let local = place.local.to_raw() as usize;
            if !defs.contains(local) {
                uses.insert(local);
            }
        }
        Rvalue::BinaryOp(_, pair) => {
            let (left, right) = pair.as_ref();
            collect_operand_uses(left, uses, defs);
            collect_operand_uses(right, uses, defs);
        }
        Rvalue::UnaryOp(_, operand) => collect_operand_uses(operand, uses, defs),
        Rvalue::Aggregate(_, operands) => {
            for op in operands {
                collect_operand_uses(op, uses, defs);
            }
        }
        Rvalue::Discriminant(place) | Rvalue::Len(place) => {
            let local = place.local.to_raw() as usize;
            if !defs.contains(local) {
                uses.insert(local);
            }
        }
        Rvalue::Cast(_, operand, _) => collect_operand_uses(operand, uses, defs),
        Rvalue::Repeat(operand, _) => collect_operand_uses(operand, uses, defs),
    }
}

fn collect_operand_uses(operand: &Operand, uses: &mut BitSet, defs: &BitSet) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            let local = place.local.to_raw() as usize;
            if !defs.contains(local) {
                uses.insert(local);
            }
        }
        Operand::Constant(_) => {}
    }
}

fn collect_terminator_uses(kind: &TerminatorKind, uses: &mut BitSet, defs: &mut BitSet) {
    match kind {
        TerminatorKind::Goto { .. } | TerminatorKind::Return | TerminatorKind::Unreachable => {}
        TerminatorKind::SwitchInt { discr, .. } => {
            collect_operand_uses(discr, uses, defs);
        }
        TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } => {
            collect_operand_uses(func, uses, defs);
            for arg in args {
                collect_operand_uses(arg, uses, defs);
            }
            defs.insert(destination.local.to_raw() as usize);
        }
        TerminatorKind::Assert { cond, .. } => {
            collect_operand_uses(cond, uses, defs);
        }
        TerminatorKind::Drop { place, .. } => {
            let local = place.local.to_raw() as usize;
            if !defs.contains(local) {
                uses.insert(local);
            }
        }
    }
}

/// Return the successor basic blocks of a terminator.
fn successor_blocks(kind: &TerminatorKind) -> Vec<BasicBlockIdx> {
    match kind {
        TerminatorKind::Goto { target } => vec![*target],
        TerminatorKind::SwitchInt { targets, .. } => {
            let mut succs: Vec<BasicBlockIdx> = targets.iter().map(|(_, bb)| bb).collect();
            succs.push(targets.otherwise());
            succs
        }
        TerminatorKind::Return | TerminatorKind::Unreachable => vec![],
        TerminatorKind::Call {
            target, cleanup, ..
        } => {
            let mut succs = Vec::new();
            if let Some(t) = target {
                succs.push(*t);
            }
            if let Some(c) = cleanup {
                succs.push(*c);
            }
            succs
        }
        TerminatorKind::Assert {
            target, cleanup, ..
        } => {
            let mut succs = vec![*target];
            if let Some(c) = cleanup {
                succs.push(*c);
            }
            succs
        }
        TerminatorKind::Drop {
            target, cleanup, ..
        } => {
            let mut succs = vec![*target];
            if let Some(c) = cleanup {
                succs.push(*c);
            }
            succs
        }
    }
}

// ---------------------------------------------------------------------------
// Per-statement liveness within a single basic block
// ---------------------------------------------------------------------------

/// Compute per-statement liveness within a basic block by scanning
/// backward from `live_out`.
///
/// Returns a vector where `result[i]` is the set of locals live just
/// **before** statement `i` executes.  `result[num_stmts]` equals
/// `live_out` (live just after the last statement, before the terminator).
fn compute_stmt_liveness(body: &Body, block: BasicBlockIdx, live_out: &BitSet) -> Vec<BitSet> {
    let block_data = &body.basic_blocks[block];
    let num_stmts = block_data.statements.len();
    let num_locals = body.locals.len();

    let mut liveness = vec![BitSet::with_capacity(num_locals); num_stmts + 1];
    liveness[num_stmts] = live_out.clone();

    for i in (0..num_stmts).rev() {
        let mut current = liveness[i + 1].clone();
        let stmt = &block_data.statements[i];
        if let StatementKind::Assign(place, rvalue) = &stmt.kind {
            // Kill: the destination is defined here.
            current.remove(place.local.to_raw() as usize);
            // Gen: all locals read by the rvalue.
            gen_rvalue_uses(rvalue, &mut current);
        }
        liveness[i] = current;
    }

    liveness
}

/// Add all locals read by an rvalue to the liveness set.
fn gen_rvalue_uses(rvalue: &Rvalue, live: &mut BitSet) {
    match rvalue {
        Rvalue::Use(operand) => gen_operand_uses(operand, live),
        Rvalue::Ref(place, _) => {
            live.insert(place.local.to_raw() as usize);
        }
        Rvalue::BinaryOp(_, pair) => {
            let (left, right) = pair.as_ref();
            gen_operand_uses(left, live);
            gen_operand_uses(right, live);
        }
        Rvalue::UnaryOp(_, operand) => gen_operand_uses(operand, live),
        Rvalue::Aggregate(_, operands) => {
            for op in operands {
                gen_operand_uses(op, live);
            }
        }
        Rvalue::Discriminant(place) | Rvalue::Len(place) => {
            live.insert(place.local.to_raw() as usize);
        }
        Rvalue::Cast(_, operand, _) => gen_operand_uses(operand, live),
        Rvalue::Repeat(operand, _) => gen_operand_uses(operand, live),
    }
}

fn gen_operand_uses(operand: &Operand, live: &mut BitSet) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            live.insert(place.local.to_raw() as usize);
        }
        Operand::Constant(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Conflict detection
// ---------------------------------------------------------------------------

/// Check for borrow conflicts at a single statement.
///
/// `active_loans` contains all loans whose `dest_local` is live at this
/// program point, meaning the reference is still in use and the borrow
/// is still active.
fn check_stmt_conflicts(
    stmt: &glyim_mir::Statement,
    active_loans: &[&Loan],
    errors: &mut Vec<GlyimDiagnostic>,
) {
    match &stmt.kind {
        StatementKind::Assign(dest, rvalue) => {
            match rvalue {
                Rvalue::Ref(borrowed, kind) => {
                    // Creating a new borrow — check if it conflicts with
                    // any *already active* loan on the same place.
                    let borrowed_local = borrowed.local;
                    for loan in active_loans {
                        if loan.borrowed_local == borrowed_local {
                            let conflict = conflicts_with_active(&loan.kind, kind);
                            if conflict {
                                let msg = format!(
                                    "cannot borrow `{}` as {} because it is also borrowed as {}",
                                    borrowed_local.to_raw(),
                                    borrow_kind_str(kind),
                                    borrow_kind_str(&loan.kind)
                                );
                                let mut diag =
                                    GlyimDiagnostic::borrow_error(stmt.source_info.span, msg);
                                diag = diag.with_sub(SubDiagnostic {
                                    severity: DiagSeverity::Note,
                                    message: format!(
                                        "previous {} borrow occurs here",
                                        borrow_kind_str(&loan.kind)
                                    ),
                                    span: Some(MultiSpan::from_span(loan.span)),
                                });
                                errors.push(diag);
                            }
                        }
                    }
                }
                _ => {
                    // Not a borrow creation.  Check two things:
                    // (a) reads of a place that is mutably/unique-borrowed
                    // (b) writes to a place that is borrowed at all

                    // (a) Read conflicts
                    let read_locals = collect_rvalue_read_locals(rvalue);
                    for read_local in read_locals {
                        for loan in active_loans {
                            if loan.borrowed_local == read_local
                                && matches!(loan.kind, BorrowKind::Mut { .. } | BorrowKind::Unique)
                            {
                                let msg = format!(
                                    "cannot use `{}` because it is {} borrowed",
                                    read_local.to_raw(),
                                    borrow_kind_str(&loan.kind)
                                );
                                let mut diag =
                                    GlyimDiagnostic::borrow_error(stmt.source_info.span, msg);
                                diag = diag.with_sub(SubDiagnostic {
                                    severity: DiagSeverity::Note,
                                    message: format!(
                                        "{} borrow occurs here",
                                        borrow_kind_str(&loan.kind)
                                    ),
                                    span: Some(MultiSpan::from_span(loan.span)),
                                });
                                errors.push(diag);
                            }
                        }
                    }

                    // (b) Write conflicts — assigning to a borrowed place
                    let dest_local = dest.local;
                    for loan in active_loans {
                        if loan.borrowed_local == dest_local {
                            let msg = format!(
                                "cannot assign to `{}` because it is borrowed",
                                dest_local.to_raw()
                            );
                            let mut diag =
                                GlyimDiagnostic::borrow_error(stmt.source_info.span, msg);
                            diag = diag.with_sub(SubDiagnostic {
                                severity: DiagSeverity::Note,
                                message: format!(
                                    "{} borrow occurs here",
                                    borrow_kind_str(&loan.kind)
                                ),
                                span: Some(MultiSpan::from_span(loan.span)),
                            });
                            errors.push(diag);
                        }
                    }
                }
            }
        }
        StatementKind::StorageLive(_) | StatementKind::StorageDead(_) | StatementKind::Nop => {}
    }
}

/// Does creating a borrow of `new_kind` conflict with an already active
/// borrow of `active_kind` on the same place?
fn conflicts_with_active(active_kind: &BorrowKind, new_kind: &BorrowKind) -> bool {
    match (active_kind, new_kind) {
        (BorrowKind::Shared, BorrowKind::Shared) => false,
        (BorrowKind::Mut { .. }, _) | (_, BorrowKind::Mut { .. }) => true,
        (BorrowKind::Unique, _) | (_, BorrowKind::Unique) => true,
    }
}

/// Collect the root locals read by an rvalue (excluding `Rvalue::Ref`,
/// which is handled separately as a borrow creation).
fn collect_rvalue_read_locals(rvalue: &Rvalue) -> Vec<LocalIdx> {
    let mut locals = Vec::new();
    match rvalue {
        Rvalue::Use(operand) => collect_operand_read_locals(operand, &mut locals),
        Rvalue::Ref(_, _) => { /* handled separately */ }
        Rvalue::BinaryOp(_, pair) => {
            let (left, right) = pair.as_ref();
            collect_operand_read_locals(left, &mut locals);
            collect_operand_read_locals(right, &mut locals);
        }
        Rvalue::UnaryOp(_, operand) => collect_operand_read_locals(operand, &mut locals),
        Rvalue::Aggregate(_, operands) => {
            for op in operands {
                collect_operand_read_locals(op, &mut locals);
            }
        }
        Rvalue::Discriminant(place) | Rvalue::Len(place) => {
            locals.push(place.local);
        }
        Rvalue::Cast(_, operand, _) => collect_operand_read_locals(operand, &mut locals),
        Rvalue::Repeat(operand, _) => collect_operand_read_locals(operand, &mut locals),
    }
    locals
}

fn collect_operand_read_locals(operand: &Operand, locals: &mut Vec<LocalIdx>) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            locals.push(place.local);
        }
        Operand::Constant(_) => {}
    }
}

fn borrow_kind_str(kind: &BorrowKind) -> &'static str {
    match kind {
        BorrowKind::Shared => "shared",
        BorrowKind::Mut { .. } => "mutable",
        BorrowKind::Unique => "unique",
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Check borrow conflicts using Polonius-style region inference.
///
/// The analysis:
/// 1. Collects all loans (borrows) from the MIR body.
/// 2. Computes local liveness across the CFG.
/// 3. Walks each basic block, determining active loans at each statement,
///    and flags conflicts between active loans and place accesses.
pub fn check_borrows(_ctx: &dyn BorrowckCtx, body: &Body) -> BorrowckResult {
    let loans = collect_loans(body);
    let liveness = compute_liveness(body);
    let mut errors = Vec::new();

    for (block_idx, block_data) in body.basic_blocks.iter_enumerated() {
        let block_usize = block_idx.to_raw() as usize;
        let live_out = &liveness.live_out[block_usize];
        let stmt_liveness = compute_stmt_liveness(body, block_idx, live_out);

        for (stmt_idx, stmt) in block_data.statements.iter().enumerate() {
            let live_locals = &stmt_liveness[stmt_idx];

            // A loan is active at this point if the local holding the
            // reference is live (will be used later).
            let active_loans: Vec<&Loan> = loans
                .iter()
                .filter(|loan| live_locals.contains(loan.dest_local.to_raw() as usize))
                .collect();

            check_stmt_conflicts(stmt, &active_loans, &mut errors);
        }
    }

    BorrowckResult { errors }
}

#[cfg(test)]
mod tests;
