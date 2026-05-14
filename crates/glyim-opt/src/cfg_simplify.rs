use glyim_mir::*;
use glyim_core::IndexVec;
use glyim_span::Span;
use glyim_type::TyCtx;

/// Simplifies the control‑flow graph by merging basic blocks that are connected
/// via a single unconditional Goto edge.
pub(crate) fn run(_ctx: &TyCtx, body: &mut Body) {
    let mut blocks: Vec<BasicBlockData> = body.basic_blocks.clone().into_raw();
    let mut deleted = vec![false; blocks.len()];
    let mut changed = true;

    while changed {
        changed = false;
        let mut preds: Vec<Vec<usize>> = vec![Vec::new(); blocks.len()];
        for (i, block) in blocks.iter().enumerate() {
            if deleted[i] {
                continue;
            }
            for succ in terminator_successors(&block.terminator) {
                let si = succ.to_raw() as usize;
                if si < blocks.len() {
                    preds[si].push(i);
                }
            }
        }

        for bb in 1..blocks.len() {
            if deleted[bb] {
                continue;
            }
            if preds[bb].len() != 1 {
                continue;
            }
            let pred = preds[bb][0];
            if deleted[pred] {
                continue;
            }
            if !matches!(&blocks[pred].terminator.kind, TerminatorKind::Goto { target } if target.to_raw() as usize == bb)
            {
                continue;
            }

            let mut bb_stmts = std::mem::take(&mut blocks[bb].statements);
            blocks[pred].statements.append(&mut bb_stmts);

            let dummy_term = Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(Span::DUMMY),
            };
            let old_bb_term = std::mem::replace(&mut blocks[bb].terminator, dummy_term);
            blocks[pred].terminator = old_bb_term;
            deleted[bb] = true;
            changed = true;
            break;
        }
    }

    let mut new_blocks = Vec::new();
    let mut old_to_new: Vec<Option<usize>> = vec![None; blocks.len()];
    for (i, block) in blocks.into_iter().enumerate() {
        if !deleted[i] {
            old_to_new[i] = Some(new_blocks.len());
            new_blocks.push(block);
        }
    }

    for block in &mut new_blocks {
        remap_terminator(block, &old_to_new);
    }

    body.basic_blocks = IndexVec::from_raw(new_blocks);
}

pub(crate) fn terminator_successors(term: &Terminator) -> Vec<BasicBlockIdx> {
    match &term.kind {
        TerminatorKind::Goto { target } => vec![*target],
        TerminatorKind::SwitchInt { targets, .. } => {
            let mut succs: Vec<_> = targets.iter().map(|(_, t)| t).collect();
            succs.push(targets.otherwise());
            succs
        }
        TerminatorKind::Call {
            func: _,
            args: _,
            destination: _,
            target,
            cleanup,
        } => {
            let mut s = Vec::new();
            if let Some(t) = target {
                s.push(*t);
            }
            if let Some(c) = cleanup {
                s.push(*c);
            }
            s
        }
        TerminatorKind::Assert {
            cond: _,
            expected: _,
            target,
            cleanup,
            msg: _,
        } => {
            let mut s = vec![*target];
            if let Some(c) = cleanup {
                s.push(*c);
            }
            s
        }
        TerminatorKind::Drop {
            place: _,
            target,
            cleanup,
        } => {
            let mut s = vec![*target];
            if let Some(c) = cleanup {
                s.push(*c);
            }
            s
        }
        _ => vec![],
    }
}

pub(crate) fn remap_terminator(block: &mut BasicBlockData, old_to_new: &[Option<usize>]) {
    let map = |idx: &BasicBlockIdx| -> BasicBlockIdx {
        let old = idx.to_raw() as usize;
        if let Some(&Some(new)) = old_to_new.get(old) {
            BasicBlockIdx::from_raw(new as u32)
        } else {
            BasicBlockIdx::from_raw(0)
        }
    };

    let map_opt = |opt: &Option<BasicBlockIdx>| -> Option<BasicBlockIdx> {
        opt.as_ref().map(&map)
    };

    match &mut block.terminator.kind {
        TerminatorKind::Goto { target } => *target = map(target),
        TerminatorKind::SwitchInt { targets, .. } => {
            let otherwise = map(&targets.otherwise());
            let values: Vec<(u128, BasicBlockIdx)> = targets
                .iter()
                .map(|(val, t)| (val, map(&t)))
                .collect();
            *targets = SwitchTargets::new(values.into_boxed_slice(), otherwise);
        }
        TerminatorKind::Call {
            func: _,
            args: _,
            destination: _,
            target,
            cleanup,
        } => {
            *target = map_opt(target);
            *cleanup = map_opt(cleanup);
        }
        TerminatorKind::Assert {
            cond: _,
            expected: _,
            target,
            cleanup,
            msg: _,
        } => {
            *target = map(target);
            *cleanup = map_opt(cleanup);
        }
        TerminatorKind::Drop {
            place: _,
            target,
            cleanup,
        } => {
            *target = map(target);
            *cleanup = map_opt(cleanup);
        }
        _ => {}
    }
}
