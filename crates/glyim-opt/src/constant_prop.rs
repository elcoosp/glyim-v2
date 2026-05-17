use glyim_mir::*;
use glyim_type::TyCtx;
use std::collections::HashMap;

/// Intra‑procedural constant propagation with bounded fixpoint iteration.
pub(crate) fn run(_ctx: &TyCtx, body: &mut Body) {
    let mut const_map: HashMap<LocalIdx, MirConst> = HashMap::new();
    let mut changed = true;
    let mut iteration = 0;
    const MAX_ITERATIONS: usize = 1000;

    while changed && iteration < MAX_ITERATIONS {
        changed = false;
        iteration += 1;
        for bb in 0..body.basic_blocks.len() {
            let block = &mut body.basic_blocks[BasicBlockIdx::from_raw(bb as u32)];
            for stmt in &mut block.statements {
                // Replace operands using the current const_map
                if let StatementKind::Assign(_, rvalue) = &mut stmt.kind {
                    let made_change = replace_in_rvalue(rvalue, &const_map);
                    changed = changed || made_change;
                }
                // Update const_map from this assignment
                if let StatementKind::Assign(place, rvalue) = &stmt.kind {
                    if place.projection.is_empty() {
                        if let Rvalue::Use(Operand::Constant(c)) = rvalue {
                            const_map.insert(place.local, c.clone());
                            // New constant may enable further propagation
                            changed = true;
                        } else {
                            if const_map.remove(&place.local).is_some() {
                                changed = true;
                            }
                        }
                    } else {
                        if const_map.remove(&place.local).is_some() {
                            changed = true;
                        }
                    }
                }
            }
        }
    }
    if iteration >= MAX_ITERATIONS {
        // Warn without tracing (tracing not in dependencies)
        #[cfg(debug_assertions)]
        eprintln!("Constant propagation reached iteration limit; possible non-termination");
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
        Rvalue::Ref(place, _) => {
            if place.projection.is_empty() && map.contains_key(&place.local) {
                // Not replacing a reference; but we still need to consider that
                // the referenced place might be a constant? We don't replace.
                false
            } else {
                false
            }
        }
        Rvalue::Aggregate(_, operands) => {
            let mut changed = false;
            for op in operands {
                changed = replace_operand(op, map) || changed;
            }
            changed
        }
        Rvalue::Discriminant(place) | Rvalue::Len(place) => {
            if place.projection.is_empty() && map.contains_key(&place.local) {
                // Cannot replace discriminant or len with constant
                false
            } else {
                false
            }
        }
        Rvalue::Cast(_, op, _) => replace_operand(op, map),
        Rvalue::Repeat(op, _) => replace_operand(op, map),
        _ => false,
    }
}
