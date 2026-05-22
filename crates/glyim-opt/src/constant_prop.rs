//! Constant propagation with dataflow analysis and cross-block propagation.
//! Uses a worklist algorithm to propagate constant values across basic blocks.

use glyim_mir::*;
use glyim_type::TyCtx;
use std::collections::{HashMap, VecDeque};

type BlockMap = HashMap<LocalIdx, Option<MirConst>>;

/// Compare two MirConst values for equality (since MirConst does not implement PartialEq).
fn const_eq(a: &MirConst, b: &MirConst) -> bool {
    // Compare kind and ty (we ignore span for propagation)
    if std::mem::discriminant(&a.kind) != std::mem::discriminant(&b.kind) {
        return false;
    }
    if a.ty != b.ty {
        return false;
    }
    match (&a.kind, &b.kind) {
        (MirConstKind::Int(a_val), MirConstKind::Int(b_val)) => a_val == b_val,
        (MirConstKind::Uint(a_val), MirConstKind::Uint(b_val)) => a_val == b_val,
        (MirConstKind::Bool(a_val), MirConstKind::Bool(b_val)) => a_val == b_val,
        (MirConstKind::Char(a_val), MirConstKind::Char(b_val)) => a_val == b_val,
        (MirConstKind::Unit, MirConstKind::Unit) => true,
        _ => false,
    }
}

/// Check if two block maps are equal (used for fixed-point convergence).
fn maps_equal(a: &BlockMap, b: &BlockMap) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (k, v) in a {
        match b.get(k) {
            None => return false,
            Some(bv) => match (v, bv) {
                (None, None) => continue,
                (Some(c1), Some(c2)) => if !const_eq(c1, c2) { return false; }
                _ => return false,
            }
        }
    }
    true
}

/// Merge two block maps at a join point. If a local has conflicting constant values
/// from different predecessors, it becomes `None` (unknown).
fn merge_maps(mut into: BlockMap, other: &BlockMap) -> BlockMap {
    for (local, other_val) in other {
        match into.get(local) {
            None => {
                into.insert(*local, other_val.clone());
            }
            Some(existing) => {
                // Check equality using custom const_eq if needed
                let equal = match (existing, other_val) {
                    (None, None) => true,
                    (Some(e), Some(o)) => const_eq(e, o),
                    _ => false,
                };
                if !equal {
                    into.insert(*local, None);
                }
            }
        }
    }
    into
}

/// Try to evaluate an Rvalue to a constant given current known locals.
fn evaluate_rvalue_to_const(
    rv: &Rvalue,
    locals: &BlockMap,
    _ctx: &TyCtx,
) -> Option<MirConst> {
    match rv {
        Rvalue::Use(op) => operand_to_const(op, locals),
        Rvalue::BinaryOp(op, box_ops) => {
            let left = operand_to_const(&box_ops.0, locals)?;
            let right = operand_to_const(&box_ops.1, locals)?;
            match (left.kind, right.kind) {
                (MirConstKind::Int(l), MirConstKind::Int(r)) => {
                    let result = match op {
                        glyim_core::primitives::BinOp::Add => l + r,
                        glyim_core::primitives::BinOp::Sub => l - r,
                        glyim_core::primitives::BinOp::Mul => l * r,
                        glyim_core::primitives::BinOp::Div => if r != 0 { l / r } else { 0 },
                        glyim_core::primitives::BinOp::Rem => if r != 0 { l % r } else { 0 },
                        _ => return None,
                    };
                    Some(MirConst {
                        kind: MirConstKind::Int(result),
                        ty: left.ty,
                        span: left.span,
                    })
                }
                _ => None,
            }
        }
        Rvalue::UnaryOp(op, operand) => {
            let c = operand_to_const(operand, locals)?;
            match c.kind {
                MirConstKind::Int(v) => {
                    let result = match op {
                        glyim_core::primitives::UnOp::Neg => -v,
                        glyim_core::primitives::UnOp::Not => !v,
                        _ => return None,
                    };
                    Some(MirConst {
                        kind: MirConstKind::Int(result),
                        ty: c.ty,
                        span: c.span,
                    })
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn operand_to_const(op: &Operand, locals: &BlockMap) -> Option<MirConst> {
    match op {
        Operand::Constant(c) => Some(c.clone()),
        Operand::Copy(place) | Operand::Move(place) => {
            if place.projection.is_empty() {
                locals.get(&place.local).and_then(|opt| opt.clone())
            } else {
                None
            }
        }
    }
}

/// Public entry point: run constant propagation on the MIR body.
pub(crate) fn run(ctx: &TyCtx, body: &mut Body) {
    let num_blocks = body.basic_blocks.len();
    if num_blocks == 0 {
        return;
    }

    // Build predecessor list
    let mut preds = vec![Vec::new(); num_blocks];
    for i in 0..num_blocks {
        let bb = BasicBlockIdx::from_raw(i as u32);
        let block = &body.basic_blocks[bb];
        for succ in super::cfg_simplify::terminator_successors(&block.terminator) {
            let succ_idx = succ.to_raw() as usize;
            if succ_idx < num_blocks {
                preds[succ_idx].push(i);
            }
        }
    }

    // Worklist of block indices that need processing
    let mut in_maps: Vec<Option<BlockMap>> = vec![None; num_blocks];
    let mut worklist = VecDeque::new();
    // Entry block: start with empty map
    in_maps[0] = Some(BlockMap::new());
    worklist.push_back(0);

    // Fixed-point iteration
    while let Some(bb_idx) = worklist.pop_front() {
        // Compute incoming map by merging all predecessor out maps
        let mut incoming = BlockMap::new();
        for &pred_idx in &preds[bb_idx] {
            if let Some(ref pred_out) = in_maps[pred_idx] {
                incoming = merge_maps(incoming, pred_out);
            }
        }
        // If no predecessor has a map yet and it's not the entry, skip (will be revisited later)
        if preds[bb_idx].is_empty() && bb_idx != 0 {
            continue;
        }

        // Transfer function: simulate block to produce outgoing map
        let mut out = incoming.clone();
        let block = &body.basic_blocks[BasicBlockIdx::from_raw(bb_idx as u32)];
        for stmt in &block.statements {
            if let StatementKind::Assign(place, rvalue) = &stmt.kind {
                // Writing to a local kills previous knowledge
                out.remove(&place.local);
                if place.projection.is_empty() {
                    if let Some(c) = evaluate_rvalue_to_const(rvalue, &out, ctx) {
                        out.insert(place.local, Some(c));
                    }
                }
            }
        }

        // Check if outgoing map changed
        let changed = match &in_maps[bb_idx] {
            None => true,
            Some(old) => !maps_equal(old, &out),
        };
        if changed {
            in_maps[bb_idx] = Some(out);
            // Propagate to successors
            let term = &body.basic_blocks[BasicBlockIdx::from_raw(bb_idx as u32)].terminator;
            for succ in super::cfg_simplify::terminator_successors(term) {
                let succ_idx = succ.to_raw() as usize;
                if !worklist.contains(&succ_idx) {
                    worklist.push_back(succ_idx);
                }
            }
        }
    }

    // Now rewrite the MIR using the final in_maps (which are the entry maps for each block)
    for bb_idx in 0..num_blocks {
        if let Some(map) = &in_maps[bb_idx] {
            let block = &mut body.basic_blocks[BasicBlockIdx::from_raw(bb_idx as u32)];
            for stmt in &mut block.statements {
                if let StatementKind::Assign(_place, rvalue) = &mut stmt.kind {
                    // Replace operands with constants
                    replace_in_rvalue(rvalue, map);
                    // Fold constant expressions (e.g., 2+3 -> 5)
                    fold_rvalue(rvalue);
                }
            }
        }
    }
}

/// Helper: replace Copy/Move operands with known constants from the map.
fn replace_operand(op: &mut Operand, locals: &BlockMap) -> bool {
    match op {
        Operand::Copy(place) | Operand::Move(place) => {
            if place.projection.is_empty() {
                if let Some(Some(c)) = locals.get(&place.local) {
                    *op = Operand::Constant(c.clone());
                    return true;
                }
            }
            false
        }
        Operand::Constant(_) => false,
    }
}

fn replace_in_rvalue(rv: &mut Rvalue, locals: &BlockMap) -> bool {
    match rv {
        Rvalue::Use(op) => replace_operand(op, locals),
        Rvalue::BinaryOp(_, box_ops) => {
            let a = replace_operand(&mut box_ops.0, locals);
            let b = replace_operand(&mut box_ops.1, locals);
            a || b
        }
        Rvalue::UnaryOp(_, op) => replace_operand(op, locals),
        Rvalue::Ref(_, _) => false,
        Rvalue::Aggregate(_, operands) => {
            let mut changed = false;
            for op in operands {
                changed = replace_operand(op, locals) || changed;
            }
            changed
        }
        Rvalue::Discriminant(_) | Rvalue::Len(_) => false,
        Rvalue::Cast(_, op, _) => replace_operand(op, locals),
        Rvalue::Repeat(op, _) => replace_operand(op, locals),
    }
}

/// Fold a binary or unary operation into a constant if all operands are constants.
fn fold_rvalue(rv: &mut Rvalue) -> bool {
    match rv {
        Rvalue::BinaryOp(op, box_ops) => {
            let left = &box_ops.0;
            let right = &box_ops.1;
            if let (Operand::Constant(lc), Operand::Constant(rc)) = (left, right) {
                if let (MirConstKind::Int(l_int), MirConstKind::Int(r_int)) = (&lc.kind, &rc.kind) {
                    let result = match op {
                        glyim_core::primitives::BinOp::Add => *l_int + *r_int,
                        glyim_core::primitives::BinOp::Sub => *l_int - *r_int,
                        glyim_core::primitives::BinOp::Mul => *l_int * *r_int,
                        glyim_core::primitives::BinOp::Div => {
                            if *r_int != 0 { *l_int / *r_int } else { 0 }
                        }
                        glyim_core::primitives::BinOp::Rem => {
                            if *r_int != 0 { *l_int % *r_int } else { 0 }
                        }
                        _ => return false,
                    };
                    let result_const = MirConst {
                        kind: MirConstKind::Int(result),
                        ty: lc.ty.clone(),
                        span: lc.span,
                    };
                    *rv = Rvalue::Use(Operand::Constant(result_const));
                    return true;
                }
            }
            false
        }
        Rvalue::UnaryOp(op, operand) => {
            if let Operand::Constant(c) = operand {
                if let MirConstKind::Int(val) = c.kind {
                    let result = match op {
                        glyim_core::primitives::UnOp::Neg => -val,
                        glyim_core::primitives::UnOp::Not => !val,
                        _ => return false,
                    };
                    let result_const = MirConst {
                        kind: MirConstKind::Int(result),
                        ty: c.ty.clone(),
                        span: c.span,
                    };
                    *rv = Rvalue::Use(Operand::Constant(result_const));
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}
