#![allow(clippy::needless_range_loop)]
#![allow(clippy::unused_enumerate_index)]
//! Drop elaboration: inserts drop flags and conditional branches around `Drop` terminators.
//! Array drops are currently replaced with a direct Goto (stub) and will be implemented fully later.

use std::collections::VecDeque;

use glyim_core::IndexVec;
use glyim_core::Mutability;
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{Ty, TyCtx, TyKind};

// -----------------------------------------------------------------------------
// Dataflow: which locals are definitely initialized at each program point.
// -----------------------------------------------------------------------------

struct MaybeInitialized {
    entry: Vec<Vec<bool>>,
}

impl MaybeInitialized {
    fn compute(body: &Body) -> Self {
        let num_locals = body.locals.len();
        let num_blocks = body.basic_blocks.len();
        let mut entry = vec![vec![false; num_locals]; num_blocks];
        for i in 0..=body.arg_count {
            entry[0][i] = true;
        }
        let mut queue = VecDeque::new();
        let mut changed = vec![true; num_blocks];
        queue.push_back(0);
        while let Some(bb_idx) = queue.pop_front() {
            let mut cur = entry[bb_idx].clone();
            let block = &body.basic_blocks[BasicBlockIdx::from_raw(bb_idx as u32)];
            for stmt in &block.statements {
                match &stmt.kind {
                    StatementKind::Assign(place, _) if place.projection.is_empty() => {
                        cur[place.local.to_raw() as usize] = true;
                    }
                    StatementKind::StorageLive(local) => {
                        cur[local.to_raw() as usize] = true;
                    }
                    StatementKind::StorageDead(local) => {
                        cur[local.to_raw() as usize] = false;
                    }
                    _ => {}
                }
            }
            for succ in super::cfg_simplify::terminator_successors(&block.terminator) {
                let succ_idx = succ.to_raw() as usize;
                let succ_entry = &mut entry[succ_idx];
                let mut changed_succ = false;
                for i in 0..num_locals {
                    if cur[i] && !succ_entry[i] {
                        succ_entry[i] = true;
                        changed_succ = true;
                    }
                }
                if changed_succ && changed[succ_idx] {
                    changed[succ_idx] = true;
                    queue.push_back(succ_idx);
                }
            }
        }
        MaybeInitialized { entry }
    }

    fn is_definitely_initialized(&self, block: BasicBlockIdx, local: LocalIdx) -> bool {
        self.entry[block.to_raw() as usize][local.to_raw() as usize]
    }
}

// -----------------------------------------------------------------------------
// Drop flags
// -----------------------------------------------------------------------------

struct DropFlags {
    flag_for_local: Vec<Option<LocalIdx>>,
}

impl DropFlags {
    fn new(ctx: &TyCtx, body: &Body, _analysis: &MaybeInitialized) -> Self {
        let mut flags = vec![None; body.locals.len()];
        for (local, decl) in body.locals.iter_enumerated() {
            if needs_drop(ctx, decl.ty) {
                flags[local.to_raw() as usize] = Some(LocalIdx::from_raw(0));
            }
        }
        DropFlags { flag_for_local: flags }
    }

    fn create_flags(&mut self, ctx: &TyCtx, body: &mut Body) {
        for flag_opt in self.flag_for_local.iter_mut() {
            if flag_opt.is_some() {
                let flag_local = body.locals.push(LocalDecl {
                    ty: ctx.bool_ty(),
                    mutability: Mutability::Mut,
                    source_info: SourceInfo::new(Span::DUMMY),
                });
                *flag_opt = Some(flag_local);
                let entry_block = &mut body.basic_blocks[BasicBlockIdx::from_raw(0)];
                entry_block.statements.insert(0, Statement {
                    kind: StatementKind::StorageLive(flag_local),
                    source_info: SourceInfo::new(Span::DUMMY),
                });
                let init = Statement {
                    kind: StatementKind::Assign(
                        Place::new(flag_local),
                        Rvalue::Use(Operand::Constant(MirConst {
                            kind: MirConstKind::Bool(false),
                            ty: ctx.bool_ty(),
                            span: Span::DUMMY,
                        })),
                    ),
                    source_info: SourceInfo::new(Span::DUMMY),
                };
                entry_block.statements.insert(1, init);
            }
        }
    }

    fn get_flag(&self, local: LocalIdx) -> Option<LocalIdx> {
        let idx = local.to_raw() as usize;
        if idx < self.flag_for_local.len() {
            self.flag_for_local[idx]
        } else {
            None
        }
    }

    fn set_flag_stmt(flag: LocalIdx, value: bool, span: Span, ctx: &TyCtx) -> Statement {
        Statement {
            kind: StatementKind::Assign(
                Place::new(flag),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Bool(value),
                    ty: ctx.bool_ty(),
                    span,
                })),
            ),
            source_info: SourceInfo::new(span),
        }
    }
}

// -----------------------------------------------------------------------------
// Main transformation
// -----------------------------------------------------------------------------

pub(crate) fn run(ctx: &TyCtx, body: &mut Body) {
    let analysis = MaybeInitialized::compute(body);
    let mut flags = DropFlags::new(ctx, body, &analysis);
    flags.create_flags(ctx, body);

    // Insert flag-setting after assignments
    for block_idx in 0..body.basic_blocks.len() {
        let block = &mut body.basic_blocks[BasicBlockIdx::from_raw(block_idx as u32)];
        let mut new_stmts = Vec::new();
        for stmt in block.statements.drain(..) {
            let span = stmt.source_info.span;
            let is_assign_to_local = if let StatementKind::Assign(place, _) = &stmt.kind {
                place.projection.is_empty()
            } else {
                false
            };
            let local = if is_assign_to_local {
                if let StatementKind::Assign(place, _) = &stmt.kind {
                    Some(place.local)
                } else {
                    None
                }
            } else {
                None
            };
            new_stmts.push(stmt);
            if let Some(local) = local && let Some(flag) = flags.get_flag(local) {
                new_stmts.push(DropFlags::set_flag_stmt(flag, true, span, ctx));
            }
        }
        block.statements = new_stmts;
    }

    // Transform Drop terminators
    let mut new_blocks = Vec::new();
    let mut block_map: Vec<Option<usize>> = vec![None; body.basic_blocks.len()];

    for (old_idx, old_block) in body.basic_blocks.iter().enumerate() {
        let old_bb = BasicBlockIdx::from_raw(old_idx as u32);
        let terminator = &old_block.terminator;

        let new_term = match &terminator.kind {
            TerminatorKind::Drop { place, target, cleanup } => {
                let ty = place.ty(ctx, &body.locals);
                if !needs_drop(ctx, ty) {
                    TerminatorKind::Goto { target: *target }
                } else if matches!(ctx.ty_kind(ty), TyKind::Array(_, _)) {
                    // For arrays, we need to expand to a loop; for now stub with Goto.
                    TerminatorKind::Goto { target: *target }
                } else if place.projection.is_empty() {
                    let local = place.local;
                    let definitely_init = analysis.is_definitely_initialized(old_bb, local);
                    if !definitely_init {
                        if let Some(flag_local) = flags.get_flag(local) {
                            let drop_block_idx = new_blocks.len();
                            let drop_block = BasicBlockIdx::from_raw(drop_block_idx as u32);
                            let clear_flag = DropFlags::set_flag_stmt(flag_local, false, terminator.source_info.span, ctx);
                            let drop_block_data = BasicBlockData {
                                statements: vec![clear_flag],
                                terminator: Terminator {
                                    kind: TerminatorKind::Drop {
                                        place: place.clone(),
                                        target: *target,
                                        cleanup: *cleanup,
                                    },
                                    source_info: terminator.source_info.clone(),
                                },
                                is_cleanup: old_block.is_cleanup,
                            };
                            new_blocks.push(drop_block_data);
                            TerminatorKind::SwitchInt {
                                discr: Operand::Copy(Place::new(flag_local)),
                                switch_ty: ctx.bool_ty(),
                                targets: SwitchTargets::if_switch(drop_block, *target),
                            }
                        } else {
                            TerminatorKind::Drop {
                                place: place.clone(),
                                target: *target,
                                cleanup: *cleanup,
                            }
                        }
                    } else {
                        TerminatorKind::Drop {
                            place: place.clone(),
                            target: *target,
                            cleanup: *cleanup,
                        }
                    }
                } else {
                    // For array drops (and other projections), we replace with a Goto.
                    // This is a stub; full implementation will generate a loop.
                    TerminatorKind::Goto { target: *target }
                }
            }
            _ => terminator.kind.clone(),
        };

        let new_idx = new_blocks.len();
        block_map[old_idx] = Some(new_idx);
        new_blocks.push(BasicBlockData {
            statements: old_block.statements.clone(),
            terminator: Terminator {
                kind: new_term,
                source_info: terminator.source_info.clone(),
            },
            is_cleanup: old_block.is_cleanup,
        });
    }

    for block in &mut new_blocks {
        super::cfg_simplify::remap_terminator(block, &block_map);
    }

    body.basic_blocks = IndexVec::from_raw(new_blocks);
}

fn needs_drop(ctx: &TyCtx, ty: Ty) -> bool {
    !matches!(ctx.ty_kind(ty), TyKind::Bool | TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_) | TyKind::Char | TyKind::Never | TyKind::Unit)
}


#[cfg(test)]
mod tests {}
