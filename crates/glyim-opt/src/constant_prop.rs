//! Constant propagation with dataflow analysis and cross-block propagation.
//! Uses a worklist algorithm to propagate constant values across basic blocks.
//! Supports Int, Uint, Bool, Char, and Float constants.

use glyim_mir::*;
use glyim_type::TyCtx;
use std::collections::{HashMap, VecDeque};

type BlockMap = HashMap<LocalIdx, Option<MirConst>>;

fn const_eq(a: &MirConst, b: &MirConst) -> bool {
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
        (MirConstKind::FloatBits(a_val), MirConstKind::FloatBits(b_val)) => a_val == b_val,
        (MirConstKind::Unit, MirConstKind::Unit) => true,
        _ => false,
    }
}

fn maps_equal(a: &BlockMap, b: &BlockMap) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (k, v) in a {
        match b.get(k) {
            None => return false,
            Some(bv) => match (v, bv) {
                (None, None) => continue,
                (Some(c1), Some(c2)) => {
                    if !const_eq(c1, c2) {
                        return false;
                    }
                }
                _ => return false,
            },
        }
    }
    true
}

fn merge_maps(mut into: BlockMap, other: &BlockMap) -> BlockMap {
    for (local, other_val) in other {
        match into.get(local) {
            None => {
                into.insert(*local, other_val.clone());
            }
            Some(existing) => {
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

fn evaluate_rvalue_to_const(rv: &Rvalue, locals: &BlockMap, _ctx: &TyCtx) -> Option<MirConst> {
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
                        glyim_core::primitives::BinOp::Div => {
                            if r != 0 { l / r } else { 0 }
                        }
                        glyim_core::primitives::BinOp::Rem => {
                            if r != 0 { l % r } else { 0 }
                        }
                        _ => return None,
                    };
                    Some(MirConst { kind: MirConstKind::Int(result), ty: left.ty, span: left.span })
                }
                (MirConstKind::Uint(l), MirConstKind::Uint(r)) => {
                    let result = match op {
                        glyim_core::primitives::BinOp::Add => l + r,
                        glyim_core::primitives::BinOp::Sub => l - r,
                        glyim_core::primitives::BinOp::Mul => l * r,
                        glyim_core::primitives::BinOp::Div => {
                            if r != 0 { l / r } else { 0 }
                        }
                        glyim_core::primitives::BinOp::Rem => {
                            if r != 0 { l % r } else { 0 }
                        }
                        _ => return None,
                    };
                    Some(MirConst { kind: MirConstKind::Uint(result), ty: left.ty, span: left.span })
                }
                (MirConstKind::Bool(l), MirConstKind::Bool(r)) => {
                    let result = match op {
                        glyim_core::primitives::BinOp::And => l && r,
                        glyim_core::primitives::BinOp::Or => l || r,
                        glyim_core::primitives::BinOp::Eq => l == r,
                        glyim_core::primitives::BinOp::Ne => l != r,
                        _ => return None,
                    };
                    Some(MirConst { kind: MirConstKind::Bool(result), ty: left.ty, span: left.span })
                }
                (MirConstKind::FloatBits(l), MirConstKind::FloatBits(r)) => {
                    let lf = f64::from_bits(l);
                    let rf = f64::from_bits(r);
                    let result = match op {
                        glyim_core::primitives::BinOp::Add => lf + rf,
                        glyim_core::primitives::BinOp::Sub => lf - rf,
                        glyim_core::primitives::BinOp::Mul => lf * rf,
                        glyim_core::primitives::BinOp::Div => {
                            if rf != 0.0 { lf / rf } else { 0.0 }
                        }
                        glyim_core::primitives::BinOp::Eq => if lf == rf { 1.0 } else { 0.0 },
                        glyim_core::primitives::BinOp::Ne => if lf != rf { 1.0 } else { 0.0 },
                        glyim_core::primitives::BinOp::Lt => if lf < rf { 1.0 } else { 0.0 },
                        glyim_core::primitives::BinOp::Gt => if lf > rf { 1.0 } else { 0.0 },
                        glyim_core::primitives::BinOp::LtEq => if lf <= rf { 1.0 } else { 0.0 },
                        glyim_core::primitives::BinOp::GtEq => if lf >= rf { 1.0 } else { 0.0 },
                        _ => return None,
                    };
                    // Convert result back to bits. For booleans, we produce a FloatBits of 0.0 or 1.0.
                    // This is not ideal but we keep type consistency.
                    let bits = result.to_bits();
                    Some(MirConst { kind: MirConstKind::FloatBits(bits), ty: left.ty, span: left.span })
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
                    Some(MirConst { kind: MirConstKind::Int(result), ty: c.ty, span: c.span })
                }
                MirConstKind::Uint(v) => {
                    let result = match op {
                        glyim_core::primitives::UnOp::Not => !v,
                        _ => return None,
                    };
                    Some(MirConst { kind: MirConstKind::Uint(result), ty: c.ty, span: c.span })
                }
                MirConstKind::Bool(v) => {
                    let result = match op {
                        glyim_core::primitives::UnOp::Not => !v,
                        _ => return None,
                    };
                    Some(MirConst { kind: MirConstKind::Bool(result), ty: c.ty, span: c.span })
                }
                MirConstKind::FloatBits(v) => {
                    let f = f64::from_bits(v);
                    let result = match op {
                        glyim_core::primitives::UnOp::Neg => -f,
                        _ => return None,
                    };
                    Some(MirConst { kind: MirConstKind::FloatBits(result.to_bits()), ty: c.ty, span: c.span })
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

pub(crate) fn run(ctx: &TyCtx, body: &mut Body) {
    let num_blocks = body.basic_blocks.len();
    if num_blocks == 0 {
        return;
    }

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

    let mut in_maps: Vec<Option<BlockMap>> = vec![None; num_blocks];
    let mut worklist = VecDeque::new();
    in_maps[0] = Some(BlockMap::new());
    worklist.push_back(0);

    while let Some(bb_idx) = worklist.pop_front() {
        let mut incoming = BlockMap::new();
        for &pred_idx in &preds[bb_idx] {
            if let Some(ref pred_out) = in_maps[pred_idx] {
                incoming = merge_maps(incoming, pred_out);
            }
        }
        if preds[bb_idx].is_empty() && bb_idx != 0 {
            continue;
        }

        let mut out = incoming.clone();
        let block = &body.basic_blocks[BasicBlockIdx::from_raw(bb_idx as u32)];
        for stmt in &block.statements {
            if let StatementKind::Assign(place, rvalue) = &stmt.kind {
                out.remove(&place.local);
                if place.projection.is_empty() {
                    if let Some(c) = evaluate_rvalue_to_const(rvalue, &out, ctx) {
                        out.insert(place.local, Some(c));
                    }
                }
            }
        }

        let changed = match &in_maps[bb_idx] {
            None => true,
            Some(old) => !maps_equal(old, &out),
        };
        if changed {
            in_maps[bb_idx] = Some(out);
            let term = &body.basic_blocks[BasicBlockIdx::from_raw(bb_idx as u32)].terminator;
            for succ in super::cfg_simplify::terminator_successors(term) {
                let succ_idx = succ.to_raw() as usize;
                if !worklist.contains(&succ_idx) {
                    worklist.push_back(succ_idx);
                }
            }
        }
    }

    for bb_idx in 0..num_blocks {
        if let Some(map) = &in_maps[bb_idx] {
            let block = &mut body.basic_blocks[BasicBlockIdx::from_raw(bb_idx as u32)];
            for stmt in &mut block.statements {
                if let StatementKind::Assign(_place, rvalue) = &mut stmt.kind {
                    replace_in_rvalue(rvalue, map);
                }
            }
        }
    }
}

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
