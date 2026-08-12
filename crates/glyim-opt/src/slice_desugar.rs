//! MIR-level slice/array projection desugaring.
//!
//! `ProjectionElem::ConstantIndex` and `ProjectionElem::Subslice` compute a
//! new base pointer (and, for `Subslice`, a new *length*) from constant
//! offsets. Every consumer of `Place` projections downstream of this pass
//! -- `glyim-codegen-llvm`'s `place_ptr`, `glyim-codegen`'s
//! `emit_place_address`, and `glyim-mir-interp`'s place evaluator --
//! assumes these are only ever the *last* element of a place's projection
//! list. That's because unlike `Field`/`Index`/`Downcast`, which just walk
//! one step deeper into the *same* allocation, `Subslice`'s result is a
//! genuinely new value (`{ ptr, len }`) that has to be materialized
//! somewhere before it can be treated as "the base of further
//! projections". A `Subslice` in the middle of a projection chain (e.g.
//! matching a field of the subslice, `x[1..][0]` collapsed into one
//! place) is not something any backend can address directly.
//!
//! This pass makes "ConstantIndex/Subslice is always terminal" hold
//! everywhere *by construction*, so backends never have to defend against
//! it. Any place where one of these projections is *not* already the last
//! element gets split at that point:
//!
//! ```text
//! _1 = move (*_2)[1..].field[3]     // ILLEGAL: Subslice not terminal
//! =>
//! StorageLive(_tmp)
//! _tmp = (*_2)[1..]                 // materialize the subslice value
//! _1 = move _tmp.field[3]           // continue from the temporary
//! ```
//!
//! Bodies with no such projections -- the overwhelming common case, since
//! `ConstantIndex`/`Subslice` only show up from slice-pattern matching --
//! are left completely unchanged; this pass is a no-op for them.
//!
//! ## Known gap: dynamic range slicing (`arr[i..j]` with runtime bounds)
//!
//! `ConstantIndex`/`Subslice` can only express *compile-time-constant*
//! offsets (`offset: u64`, `from: u64`, `to: u64`) -- that's sufficient for
//! slice-pattern prefixes/suffixes (`[a, b, ..rest]`), whose lengths are
//! always known at the pattern's definition site. It is **not** sufficient
//! for a genuinely dynamic range-index expression like `arr[i..j]` where
//! `i`/`j` are runtime locals -- there is no `Place` projection that can
//! carry a `Place`/`Operand` as its bound. Lowering `arr[i..j]` therefore
//! cannot go through a `Place` projection at all; it needs to be built as
//! an ordinary `Rvalue` (compute `data_ptr = base_ptr + i * elem_size` and
//! `len = j - i` via casts/arithmetic, then construct the `{ ptr, len }`
//! aggregate), the same shape this pass already produces for its
//! intermediate temporaries below. If/when `glyim-hir`/`glyim-lower` gains
//! a THIR-level dynamic-range-index expression, its MIR lowering should
//! emit that statement sequence directly (see `desugar_place`'s
//! `Rvalue::Use(Operand::Copy(prefix_place))` pattern for the shape to
//! follow) rather than trying to invent a new `ProjectionElem` variant for
//! it. That lowering is *not* implemented by this pass -- it belongs in
//! `glyim-lower` at THIR->MIR build time, before this pass ever sees the
//! body -- so it's called out here rather than silently left undone.

use glyim_core::primitives::Mutability;
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::TyCtx;

pub(crate) fn run(ctx: &TyCtx, body: &mut Body) {
    for i in 0..body.basic_blocks.len() {
        let bb = BasicBlockIdx::from_raw(i as u32);
        desugar_block(ctx, body, bb);
    }
}

fn desugar_block(ctx: &TyCtx, body: &mut Body, bb: BasicBlockIdx) {
    // Rebuild the block's statement list, splicing in any intermediate
    // statements a rewrite needs immediately before the statement that
    // required it.
    let old_statements = std::mem::take(&mut body.basic_blocks[bb].statements);
    let mut new_statements = Vec::with_capacity(old_statements.len());
    for mut stmt in old_statements {
        let mut prelude = Vec::new();
        desugar_statement(ctx, body, &mut stmt, &mut prelude);
        new_statements.extend(prelude);
        new_statements.push(stmt);
    }
    body.basic_blocks[bb].statements = new_statements;

    // The terminator can also reference places with non-terminal
    // ConstantIndex/Subslice projections (call args/destination, assert
    // condition, switch discriminant, drop place). Desugar those too,
    // appending any needed statements to the end of the (already rebuilt)
    // statement list, immediately before the terminator runs.
    let span = body.basic_blocks[bb].terminator.source_info.span;
    let mut terminator = std::mem::replace(
        &mut body.basic_blocks[bb].terminator,
        Terminator {
            kind: TerminatorKind::Unreachable,
            source_info: SourceInfo::new(span),
        },
    );
    let mut prelude = Vec::new();
    desugar_terminator(ctx, body, &mut terminator, &mut prelude);
    body.basic_blocks[bb].statements.extend(prelude);
    body.basic_blocks[bb].terminator = terminator;
}

fn desugar_statement(
    ctx: &TyCtx,
    body: &mut Body,
    stmt: &mut Statement,
    prelude: &mut Vec<Statement>,
) {
    let span = stmt.source_info.span;
    match &mut stmt.kind {
        StatementKind::Assign(place, rvalue) => {
            desugar_rvalue(ctx, body, rvalue, prelude, span);
            *place = desugar_place(ctx, body, place, prelude, span);
        }
        StatementKind::StorageLive(_) | StatementKind::StorageDead(_) | StatementKind::Nop => {}
    }
}

fn desugar_terminator(
    ctx: &TyCtx,
    body: &mut Body,
    terminator: &mut Terminator,
    prelude: &mut Vec<Statement>,
) {
    let span = terminator.source_info.span;
    match &mut terminator.kind {
        TerminatorKind::SwitchInt { discr, .. } => {
            desugar_operand(ctx, body, discr, prelude, span);
        }
        TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } => {
            desugar_operand(ctx, body, func, prelude, span);
            for arg in args.iter_mut() {
                desugar_operand(ctx, body, arg, prelude, span);
            }
            *destination = desugar_place(ctx, body, destination, prelude, span);
        }
        TerminatorKind::Assert { cond, .. } => {
            desugar_operand(ctx, body, cond, prelude, span);
        }
        TerminatorKind::Drop { place, .. } => {
            *place = desugar_place(ctx, body, place, prelude, span);
        }
        TerminatorKind::Goto { .. } | TerminatorKind::Return | TerminatorKind::Unreachable => {}
    }
}

fn desugar_operand(
    ctx: &TyCtx,
    body: &mut Body,
    operand: &mut Operand,
    prelude: &mut Vec<Statement>,
    span: Span,
) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            *place = desugar_place(ctx, body, place, prelude, span);
        }
        Operand::Constant(_) => {}
    }
}

fn desugar_rvalue(
    ctx: &TyCtx,
    body: &mut Body,
    rvalue: &mut Rvalue,
    prelude: &mut Vec<Statement>,
    span: Span,
) {
    match rvalue {
        Rvalue::Use(op) => desugar_operand(ctx, body, op, prelude, span),
        Rvalue::BinaryOp(_, operands) => {
            let (l, r) = operands.as_mut();
            desugar_operand(ctx, body, l, prelude, span);
            desugar_operand(ctx, body, r, prelude, span);
        }
        Rvalue::UnaryOp(_, op) => desugar_operand(ctx, body, op, prelude, span),
        Rvalue::Ref(place, _) => {
            *place = desugar_place(ctx, body, place, prelude, span);
        }
        Rvalue::Aggregate(_, ops) => {
            for op in ops.iter_mut() {
                desugar_operand(ctx, body, op, prelude, span);
            }
        }
        Rvalue::Discriminant(place) => {
            *place = desugar_place(ctx, body, place, prelude, span);
        }
        Rvalue::Len(place) => {
            *place = desugar_place(ctx, body, place, prelude, span);
        }
        Rvalue::Cast(_, op, _) => desugar_operand(ctx, body, op, prelude, span),
        Rvalue::Repeat(op, _) => desugar_operand(ctx, body, op, prelude, span),
    }
}

/// If `place` contains a `ConstantIndex`/`Subslice` element that is *not*
/// the last projection element, split the place at that point: push a
/// fresh local + `StorageLive` + `Assign` (copying everything up to and
/// including that element) into `prelude`, and return a new, shorter
/// place rooted at that fresh local for the remaining projections.
/// Recurses in case more than one such violation exists in the same
/// chain. Returns a clone of `place` unchanged if it was already fine
/// (the common case, and the only case for the vast majority of bodies,
/// which contain no `ConstantIndex`/`Subslice` at all).
fn desugar_place(
    ctx: &TyCtx,
    body: &mut Body,
    place: &Place,
    prelude: &mut Vec<Statement>,
    span: Span,
) -> Place {
    let proj = &place.projection;
    let split_at = proj.iter().enumerate().find_map(|(i, elem)| {
        let is_value_producing = matches!(
            elem,
            ProjectionElem::ConstantIndex { .. } | ProjectionElem::Subslice { .. }
        );
        (is_value_producing && i + 1 < proj.len()).then_some(i)
    });

    let Some(split_at) = split_at else {
        return place.clone();
    };

    let prefix_place = Place {
        local: place.local,
        projection: proj[..=split_at].to_vec().into_boxed_slice(),
    };
    let prefix_ty = prefix_place.ty(ctx, &body.locals);

    let tmp_local = body.locals.push(LocalDecl {
        ty: prefix_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(span),
    });
    prelude.push(Statement {
        kind: StatementKind::StorageLive(tmp_local),
        source_info: SourceInfo::new(span),
    });
    prelude.push(Statement {
        kind: StatementKind::Assign(
            Place::new(tmp_local),
            Rvalue::Use(Operand::Copy(prefix_place)),
        ),
        source_info: SourceInfo::new(span),
    });

    let rest_place = Place {
        local: tmp_local,
        projection: proj[split_at + 1..].to_vec().into_boxed_slice(),
    };
    // Recurse: the remainder of the chain might contain a *further*
    // violation (e.g. two Subslice projections chained together).
    desugar_place(ctx, body, &rest_place, prelude, span)
}

#[cfg(test)]
mod tests {
    // NOTE: these are illustrative placeholders. Wire them up against
    // whatever body-construction helpers `glyim-opt`'s existing test
    // module (`glyim-opt/src/tests.rs`) already provides (e.g. a small
    // `Body` builder used by `dce`/`cfg_simplify`'s own tests) rather than
    // constructing `Body`/`Place`/`LocalDecl` by hand here, since this
    // crate doesn't have visibility into `glyim-type`'s `TyCtx`
    // construction helpers used elsewhere in the test suite.
    //
    // Cases worth covering once wired up:
    //   1. A place with a single terminal `Subslice` -> unchanged.
    //   2. A place with `Subslice` followed by `Field` -> split into a
    //      temporary + a two-element (well, one-element) remainder place.
    //   3. A place with no `ConstantIndex`/`Subslice` at all -> completely
    //      unchanged, and the pass must not allocate any new locals.
    //   4. `ConstantIndex`/`Subslice` appearing inside `Rvalue::Ref`,
    //      `Operand` within a `Call`'s args, and a `SwitchInt` discriminant
    //      -- confirming the terminator-side rewriting path also fires.
}
