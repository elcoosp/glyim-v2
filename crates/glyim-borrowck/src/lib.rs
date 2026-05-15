//! Borrow checker using non-lexical lifetimes (NLL).
//!
//! **Limitation:** This implementation only tracks borrows within a single basic block.
//! Borrows that span across basic blocks are not tracked.
//!
//! A proper dataflow‑based analysis is needed for production use.

use glyim_diag::{GlyimDiagnostic, MultiSpan};
use glyim_mir::{Body, BorrowKind, Operand, Place, Rvalue, StatementKind};
use glyim_span::Span;
use glyim_type::TyCtx;

#[derive(Clone, Debug)]
pub struct BorrowckResult {
    pub errors: Vec<GlyimDiagnostic>,
}

pub trait BorrowckCtx {
    fn ty_ctx(&self) -> &TyCtx;
    fn local_decl(&self, local: glyim_mir::LocalIdx) -> &glyim_mir::LocalDecl;
    fn is_copy(&self, ty: glyim_type::Ty) -> bool;
}

/// Determine the statement indices where a given local is read.
fn collect_local_reads(body: &Body) -> Vec<Vec<usize>> {
    let local_count = body.locals.len();
    let mut reads: Vec<Vec<usize>> = vec![Vec::new(); local_count];

    for (stmt_idx, stmt) in body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .enumerate()
    {
        match &stmt.kind {
            StatementKind::Assign(_, rvalue) => {
                collect_operand_reads(rvalue, stmt_idx, &mut reads);
            }
            StatementKind::StorageLive(_) | StatementKind::StorageDead(_) | StatementKind::Nop => {}
        }
    }
    reads
}

fn collect_operand_reads(rvalue: &Rvalue, stmt_idx: usize, reads: &mut [Vec<usize>]) {
    match rvalue {
        Rvalue::Use(operand) => record_operand(operand, stmt_idx, reads),
        Rvalue::Ref(place, _) => record_place(place, stmt_idx, reads),
        Rvalue::BinaryOp(_, pair) => {
            let (left, right) = pair.as_ref();
            record_operand(left, stmt_idx, reads);
            record_operand(right, stmt_idx, reads);
        }
        Rvalue::UnaryOp(_, operand) => record_operand(operand, stmt_idx, reads),
        Rvalue::Aggregate(_, operands) => {
            for op in operands {
                record_operand(op, stmt_idx, reads);
            }
        }
        Rvalue::Discriminant(place) => record_place(place, stmt_idx, reads),
        Rvalue::Len(place) => record_place(place, stmt_idx, reads),
        Rvalue::Cast(_, operand, _) => record_operand(operand, stmt_idx, reads),
        Rvalue::Repeat(operand, _) => record_operand(operand, stmt_idx, reads),
    }
}

fn record_operand(op: &Operand, idx: usize, reads: &mut [Vec<usize>]) {
    match op {
        Operand::Copy(place) | Operand::Move(place) => record_place(place, idx, reads),
        Operand::Constant(_) => {}
    }
}

fn record_place(place: &Place, idx: usize, reads: &mut [Vec<usize>]) {
    let local = place.local;
    if (local.to_raw() as usize) < reads.len() {
        reads[local.to_raw() as usize].push(idx);
    }
}

/// Information about an active borrow.
struct ActiveBorrow {
    kind: BorrowKind,
    span: Span,
    last_use: usize,
}

/// Check borrow conflicts in the body.
pub fn check_borrows(_ctx: &dyn BorrowckCtx, body: &Body) -> BorrowckResult {
    let reads = collect_local_reads(body);

    // Compute last‑use index for each local.
    let mut last_use_by_local: Vec<Option<usize>> = vec![None; body.locals.len()];
    let mut borrow_def_stmt: Vec<Option<usize>> = vec![None; body.locals.len()];

    // First pass: record definition points for borrow destinations.
    for (stmt_idx, stmt) in body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .enumerate()
    {
        if let StatementKind::Assign(place, Rvalue::Ref(..)) = &stmt.kind {
            let dest_local = place.local;
            if (dest_local.to_raw() as usize) < borrow_def_stmt.len() {
                borrow_def_stmt[dest_local.to_raw() as usize] = Some(stmt_idx);
            }
        }
    }

    // Compute last use: max of read positions, or definition point if no reads.
    for (local, def_stmt_opt) in borrow_def_stmt.iter().enumerate() {
        if let Some(def_stmt) = def_stmt_opt {
            let max_read = reads[local].iter().copied().max();
            let last_use = max_read.unwrap_or(*def_stmt);
            last_use_by_local[local] = Some(last_use);
        }
    }

    let mut errors = Vec::new();
    let mut active_borrows: Vec<Vec<ActiveBorrow>> =
        (0..body.locals.len()).map(|_| Vec::new()).collect();

    for (stmt_idx, stmt) in body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .enumerate()
    {
        // Expire any borrows whose last use is before this statement.
        for borrows in active_borrows.iter_mut() {
            borrows.retain(|b| b.last_use >= stmt_idx);
        }

        if let StatementKind::Assign(dest_place, Rvalue::Ref(borrowed_place, kind)) = &stmt.kind {
            let borrowed_local = borrowed_place.local;
            let dest_local = dest_place.local;
            let dest_last_use = last_use_by_local
                .get(dest_local.to_raw() as usize)
                .copied()
                .flatten()
                .unwrap_or(stmt_idx);

            // Check conflicts with currently active borrows on the same place.
            let existing = &active_borrows[borrowed_local.to_raw() as usize];
            for existing_borrow in existing {
                let conflict = match (&existing_borrow.kind, kind) {
                    (BorrowKind::Shared, BorrowKind::Shared) => false,
                    (BorrowKind::Mut { .. }, _) | (_, BorrowKind::Mut { .. }) => true,
                    (BorrowKind::Unique, _) | (_, BorrowKind::Unique) => true,
                };
                if conflict {
                    let msg = format!(
                        "cannot borrow `{}` as {} because it is also borrowed as {}",
                        borrowed_local.to_raw(),
                        borrow_kind_str(kind),
                        borrow_kind_str(&existing_borrow.kind)
                    );
                    let mut diag = GlyimDiagnostic::borrow_error(stmt.source_info.span, msg);
                    diag = diag.with_sub(glyim_diag::SubDiagnostic {
                        severity: glyim_diag::DiagSeverity::Note,
                        message: format!(
                            "previous {} borrow occurs here",
                            borrow_kind_str(&existing_borrow.kind)
                        ),
                        span: Some(MultiSpan::from_span(existing_borrow.span)),
                    });
                    errors.push(diag);
                    break;
                }
            }

            active_borrows[borrowed_local.to_raw() as usize].push(ActiveBorrow {
                kind: *kind,
                span: stmt.source_info.span,
                last_use: dest_last_use,
            });
        }
    }

    BorrowckResult { errors }
}

fn borrow_kind_str(kind: &BorrowKind) -> &'static str {
    match kind {
        BorrowKind::Shared => "shared",
        BorrowKind::Mut { .. } => "mutable",
        BorrowKind::Unique => "unique",
    }
}

#[cfg(test)]
mod tests;
