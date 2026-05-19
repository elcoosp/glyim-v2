use glyim_mir::*;
use glyim_type::TyCtx;
use std::collections::HashMap;

/// Intra-procedural constant propagation.
///
/// Replaces `Copy(local)` / `Move(local)` operands with the known constant
/// value when the local was previously assigned a constant. A single forward
/// pass is sufficient for straight-line code; extra iterations handle cases
/// where propagation reveals new constants.
pub(crate) fn run(_ctx: &TyCtx, body: &mut Body) {
    let mut const_map: HashMap<LocalIdx, MirConst> = HashMap::new();
    let mut changed = true;
    let mut iteration = 0;
    const MAX_ITERATIONS: usize = 10;

    while changed && iteration < MAX_ITERATIONS {
        changed = false;
        iteration += 1;
        for bb in 0..body.basic_blocks.len() {
            let block = &mut body.basic_blocks[BasicBlockIdx::from_raw(bb as u32)];
            for stmt in &mut block.statements {
                // Phase 1: replace operands using the current const_map.
                if let StatementKind::Assign(_, rvalue) = &mut stmt.kind {
                    let made_change = replace_in_rvalue(rvalue, &const_map);
                    changed = changed || made_change;
                }
                // Phase 2: update const_map from this assignment.
                if let StatementKind::Assign(place, rvalue) = &stmt.kind {
                    if place.projection.is_empty() {
                        if let Rvalue::Use(Operand::Constant(c)) = rvalue {
                            // Only signal changed when we insert a brand-new entry.
                            let prior = const_map.insert(place.local, c.clone());
                            if prior.is_none() {
                                changed = true;
                            }
                        } else {
                            // Non-constant assignment invalidates the local.
                            const_map.remove(&place.local);
                        }
                    } else {
                        // Projection write invalidates the whole local.
                        const_map.remove(&place.local);
                    }
                }
            }
        }
    }
    if iteration >= MAX_ITERATIONS {
        #[cfg(debug_assertions)]
        eprintln!(
            "Constant propagation reached iteration limit after {} iterations",
            iteration
        );
    }
}

fn replace_operand(op: &mut Operand, map: &HashMap<LocalIdx, MirConst>) -> bool {
    match op {
        Operand::Copy(place) | Operand::Move(place) => {
            if place.projection.is_empty()
                && let Some(c) = map.get(&place.local)
            {
                *op = Operand::Constant(c.clone());
                true
            } else {
                false
            }
        }
        Operand::Constant(_) => false,
    }
}

fn replace_in_rvalue(rv: &mut Rvalue, map: &HashMap<LocalIdx, MirConst>) -> bool {
    match rv {
        Rvalue::Use(op) => replace_operand(op, map),
        Rvalue::BinaryOp(_, box_ops) => {
            let a = replace_operand(&mut box_ops.0, map);
            let b = replace_operand(&mut box_ops.1, map);
            a || b
        }
        Rvalue::UnaryOp(_, op) => replace_operand(op, map),
        Rvalue::Ref(_, _) => false,
        Rvalue::Aggregate(_, operands) => {
            let mut changed = false;
            for op in operands {
                changed = replace_operand(op, map) || changed;
            }
            changed
        }
        Rvalue::Discriminant(_) | Rvalue::Len(_) => false,
        Rvalue::Cast(_, op, _) => replace_operand(op, map),
        Rvalue::Repeat(op, _) => replace_operand(op, map),
    }
}
