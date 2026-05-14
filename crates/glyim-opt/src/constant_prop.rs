use glyim_mir::*;
use glyim_type::TyCtx;
use std::collections::HashMap;

/// Intra‑block constant propagation.
pub(crate) fn run(_ctx: &TyCtx, body: &mut Body) {
    for bb in 0..body.basic_blocks.len() {
        let block = &mut body.basic_blocks[BasicBlockIdx::from_raw(bb as u32)];
        let mut const_map: HashMap<LocalIdx, MirConst> = HashMap::new();
        for stmt in &mut block.statements {
            if let StatementKind::Assign(_, rvalue) = &mut stmt.kind {
                replace_in_rvalue(rvalue, &const_map);
            }
            if let StatementKind::Assign(place, rvalue) = &stmt.kind
                && place.projection.is_empty()
            {
                if let Rvalue::Use(Operand::Constant(c)) = rvalue {
                    const_map.insert(place.local, c.clone());
                } else {
                    const_map.remove(&place.local);
                }
            }
        }
    }
}

fn replace_operand(op: &mut Operand, map: &HashMap<LocalIdx, MirConst>) {
    match op {
        Operand::Copy(place) | Operand::Move(place) => {
            if place.projection.is_empty()
                && let Some(c) = map.get(&place.local)
            {
                *op = Operand::Constant(c.clone());
            }
        }
        Operand::Constant(_) => {}
    }
}

fn replace_in_rvalue(rv: &mut Rvalue, map: &HashMap<LocalIdx, MirConst>) {
    match rv {
        Rvalue::Use(op) => replace_operand(op, map),
        Rvalue::BinaryOp(_, box_ops) => {
            replace_operand(&mut box_ops.0, map);
            replace_operand(&mut box_ops.1, map);
        }
        _ => {}
    }
}
