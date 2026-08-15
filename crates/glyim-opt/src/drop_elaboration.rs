#![allow(clippy::needless_range_loop)]
#![allow(clippy::unused_enumerate_index)]
//! Drop elaboration: inserts drop flags and conditional branches around `Drop` terminators.
//! Array drops are replaced with a loop that drops each element.

use std::collections::VecDeque;

use glyim_core::BinOp;
use glyim_core::IndexVec;
use glyim_core::Mutability;
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{ConstKind, Ty, TyCtx, TyKind};
use glyim_type::AdtKind;

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
        DropFlags {
            flag_for_local: flags,
        }
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
                entry_block.statements.insert(
                    0,
                    Statement {
                        kind: StatementKind::StorageLive(flag_local),
                        source_info: SourceInfo::new(Span::DUMMY),
                    },
                );
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
            if let Some(local) = local
                && let Some(flag) = flags.get_flag(local)
            {
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
            TerminatorKind::Drop {
                place,
                target,
                cleanup,
            } => {
                let ty = place.ty(ctx, &body.locals);
                if !needs_drop(ctx, ty) {
                    TerminatorKind::Goto { target: *target }
                } else if let TyKind::Array(_elem_ty, count) = ctx.ty_kind(ty) {
                    // Generate loop to drop each element
                    let len = match &count.kind {
                        ConstKind::Uint(n) => *n as u64,
                        ConstKind::Int(n) => {
                            if *n >= 0 {
                                *n as u64
                            } else {
                                0
                            }
                        }
                        _ => 0,
                    };
                    if len == 0 {
                        TerminatorKind::Goto { target: *target }
                    } else {
                        // We'll create new blocks for the loop.
                        // We need to add a new local for the index.
                        let idx_local = body.locals.push(LocalDecl {
                            ty: count.ty,
                            mutability: Mutability::Mut,
                            source_info: SourceInfo::new(terminator.source_info.span),
                        });
                        let idx_place = Place::new(idx_local);

                        // We'll create blocks: init, cond, body, exit.
                        let init_block_idx = new_blocks.len();
                        let init_block = BasicBlockIdx::from_raw(init_block_idx as u32);
                        let cond_block_idx = init_block_idx + 1;
                        let cond_block = BasicBlockIdx::from_raw(cond_block_idx as u32);
                        let body_block_idx = cond_block_idx + 1;
                        let body_block = BasicBlockIdx::from_raw(body_block_idx as u32);
                        let exit_block_idx = body_block_idx + 1;
                        let exit_block = BasicBlockIdx::from_raw(exit_block_idx as u32);

                        // Init block: idx = len; goto cond
                        let init_block_data = BasicBlockData {
                            statements: vec![Statement {
                                kind: StatementKind::Assign(
                                    idx_place.clone(),
                                    Rvalue::Use(Operand::Constant(MirConst {
                                        kind: MirConstKind::Uint(len.into()),
                                        ty: count.ty,
                                        span: terminator.source_info.span,
                                    })),
                                ),
                                source_info: SourceInfo::new(terminator.source_info.span),
                            }],
                            terminator: Terminator {
                                kind: TerminatorKind::Goto { target: cond_block },
                                source_info: terminator.source_info.clone(),
                            },
                            is_cleanup: old_block.is_cleanup,
                        };
                        new_blocks.push(init_block_data);

                        // Cond block: if idx == 0 goto exit else goto body
                        let cond_block_data = BasicBlockData {
                            statements: vec![],
                            terminator: Terminator {
                                kind: TerminatorKind::SwitchInt {
                                    discr: Operand::Copy(idx_place.clone()),
                                    switch_ty: count.ty,
                                    targets: SwitchTargets::new(
                                        vec![(0, exit_block)].into_boxed_slice(),
                                        body_block,
                                    ),
                                },
                                source_info: terminator.source_info.clone(),
                            },
                            is_cleanup: old_block.is_cleanup,
                        };
                        new_blocks.push(cond_block_data);

                        // Body block: decrement idx, then drop element at idx, then goto cond
                        let dec_stmt = Statement {
                            kind: StatementKind::Assign(
                                idx_place.clone(),
                                Rvalue::BinaryOp(
                                    BinOp::Sub,
                                    Box::new((
                                        Operand::Copy(idx_place.clone()),
                                        Operand::Constant(MirConst {
                                            kind: MirConstKind::Uint(1),
                                            ty: count.ty,
                                            span: terminator.source_info.span,
                                        }),
                                    )),
                                ),
                            ),
                            source_info: SourceInfo::new(terminator.source_info.span),
                        };
                        let elem_place = Place {
                            local: place.local,
                            projection: vec![ProjectionElem::Index(idx_local)].into_boxed_slice(),
                        };
                        let body_block_data = BasicBlockData {
                            statements: vec![dec_stmt],
                            terminator: Terminator {
                                kind: TerminatorKind::Drop {
                                    place: elem_place,
                                    target: cond_block,
                                    cleanup: *cleanup,
                                },
                                source_info: terminator.source_info.clone(),
                            },
                            is_cleanup: old_block.is_cleanup,
                        };
                        new_blocks.push(body_block_data);

                        // Exit block: goto target
                        let exit_block_data = BasicBlockData {
                            statements: vec![],
                            terminator: Terminator {
                                kind: TerminatorKind::Goto { target: *target },
                                source_info: terminator.source_info.clone(),
                            },
                            is_cleanup: old_block.is_cleanup,
                        };
                        new_blocks.push(exit_block_data);

                        // Return Goto to init block
                        TerminatorKind::Goto { target: init_block }
                    }
                } else if place.projection.is_empty() {
                    // Existing logic for non-array drops
                    let local = place.local;
                    let definitely_init = analysis.is_definitely_initialized(old_bb, local);
                    if !definitely_init {
                        if let Some(flag_local) = flags.get_flag(local) {
                            let drop_block_idx = new_blocks.len();
                            let drop_block = BasicBlockIdx::from_raw(drop_block_idx as u32);
                            let clear_flag = DropFlags::set_flag_stmt(
                                flag_local,
                                false,
                                terminator.source_info.span,
                                ctx,
                            );
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
                    // Other projections: stub with Goto
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


/// Determine if a type needs drop glue.
/// A type needs drop if it implements Drop directly, or contains a field/element that needs drop.
/// Memoization is used to avoid repeated work.
fn needs_drop(ctx: &TyCtx, ty: Ty) -> bool {
    use std::collections::HashSet;
    use glyim_type::TyKind;
    use glyim_type::GenericArg;

    fn needs_drop_rec(ctx: &TyCtx, ty: Ty, visited: &mut HashSet<Ty>) -> bool {
        if visited.contains(&ty) {
            // Recursive type: assume it needs drop if it contains a field that might need drop.
            // For correctness, we conservatively return true.
            return true;
        }
        visited.insert(ty);

        match ctx.ty_kind(ty) {
            TyKind::Adt(adt_id, _substs) => {
                // Check if the ADT has a Drop impl (we don't have explicit Drop trait yet, so we check fields).
                // For now, we assume any ADT that is not a primitive and has fields might need drop.
                // But we must check its fields recursively.
                if let Some(adt_def) = ctx.adt_def(*adt_id) {
                    // If it's a union, we conservatively say it needs drop (user is responsible).
                    if adt_def.kind == AdtKind::Union {
                        return true;
                    }
                    // Check each variant's fields.
                    for variant in &adt_def.variants {
                        for field in variant.fields.iter() {
                            if needs_drop_rec(ctx, field.ty, visited) {
                                return true;
                            }
                        }
                    }
                    false
                } else {
                    // Unknown ADT: conservatively true.
                    true
                }
            }
            TyKind::Array(elem_ty, _) => {
                needs_drop_rec(ctx, *elem_ty, visited)
            }
            TyKind::Slice(elem_ty) => {
                needs_drop_rec(ctx, *elem_ty, visited)
            }
            TyKind::Tuple(substs) => {
                for arg in ctx.substitution_args(*substs) {
                    if let GenericArg::Ty(t) = arg {
                        if needs_drop_rec(ctx, *t, visited) {
                            return true;
                        }
                    }
                }
                false
            }
            TyKind::Closure(_, substs) => {
                for arg in ctx.substitution_args(*substs) {
                    if let GenericArg::Ty(t) = arg {
                        if needs_drop_rec(ctx, *t, visited) {
                            return true;
                        }
                    }
                }
                false
            }
            // Primitive types don't need drop.
            TyKind::Bool | TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_) | TyKind::Char
            | TyKind::Never | TyKind::Unit => false,
            // References and raw pointers don't need drop (the pointee is not owned).
            TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => false,
            // Function pointers and function definitions don't need drop.
            TyKind::FnPtr(_) | TyKind::FnDef(_, _) => false,
            // Opaque types, projections, etc.: conservatively true.
            _ => true,
        }
    }

    let mut visited = HashSet::new();
    needs_drop_rec(ctx, ty, &mut visited)
}

// TODO: Implement per-projection MaybeInitialized dataflow
