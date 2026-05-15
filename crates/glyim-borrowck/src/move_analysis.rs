//! Move analysis and partial move tracking.
//!
//! This module implements a dataflow analysis that tracks which places have been
//! moved out of at each program point. It supports:
//! - Whole-struct moves: moving a struct moves all fields
//! - Partial moves: moving one field of a struct leaves other fields usable
//! - Copy types: moving a copy type does not produce a move
//! - Reassignment: reassigning a moved local makes it usable again
//!
//! The analysis works at the level of `MovePath`s, which represent prefixes
//! of places that can be independently moved. For a struct with fields 0 and 1,
//! the root local has move paths for each field.

use fixedbitset::FixedBitSet as BitSet;
use glyim_diag::GlyimDiagnostic;
use glyim_mir::{
    BasicBlockIdx, Body, LocalDecl, LocalIdx, Operand, Place, ProjectionElem, Rvalue,
    StatementKind, TerminatorKind,
};
use glyim_type::TyCtx;

use crate::BorrowckCtx;

// ---------------------------------------------------------------------------
// Move path index
// ---------------------------------------------------------------------------

/// Index into the move path arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct MovePathIdx(u32);

impl MovePathIdx {
    #[inline]
    fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
    #[inline]
    fn to_raw(self) -> u32 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Move path representation
// ---------------------------------------------------------------------------

/// A move path represents a place (or prefix of a place) that can be
/// independently moved.
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct MovePath {
    /// The place this move path corresponds to.
    place: Place,
    /// Parent move path (e.g., the struct local for a field).
    parent: Option<MovePathIdx>,
    /// Child move paths (e.g., fields of a struct).
    children: Vec<MovePathIdx>,
}

/// Arena of move paths for a single MIR body.
struct MovePathArena {
    paths: Vec<MovePath>,
}

impl MovePathArena {
    fn new() -> Self {
        Self { paths: Vec::new() }
    }

    fn push(&mut self, path: MovePath) -> MovePathIdx {
        let idx = MovePathIdx::from_raw(self.paths.len() as u32);
        self.paths.push(path);
        idx
    }

    fn get(&self, idx: MovePathIdx) -> &MovePath {
        &self.paths[idx.to_raw() as usize]
    }

    fn get_mut(&mut self, idx: MovePathIdx) -> &mut MovePath {
        &mut self.paths[idx.to_raw() as usize]
    }

    fn len(&self) -> usize {
        self.paths.len()
    }

    fn clone_paths(&self) -> MovePathArena {
        MovePathArena {
            paths: self.paths.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Move data: which paths are moved at each point
// ---------------------------------------------------------------------------

/// Result of the move dataflow analysis.
#[allow(dead_code)]
pub(crate) struct MoveAnalysisResult {
    /// For each basic block, the set of move path indices that are moved
    /// on entry to that block.
    moved_in: Vec<BitSet>,
    /// For each basic block, the set of move path indices that are moved
    /// on exit from that block.
    moved_out: Vec<BitSet>,
    /// The move path arena.
    move_paths: MovePathArena,
    /// Map from local index to its root move path (if any).
    local_to_move_path: Vec<Option<MovePathIdx>>,
}

// ---------------------------------------------------------------------------
// Building move paths
// ---------------------------------------------------------------------------

/// Build the move path arena for a MIR body.
///
/// For each local, we create a root move path. For locals whose type is
/// a tuple or struct (ADT), we also create child move paths for each field.
fn build_move_paths(body: &Body, ctx: &dyn BorrowckCtx) -> MovePathArena {
    let mut arena = MovePathArena::new();

    for (local_idx, local_decl) in body.locals.iter_enumerated() {
        let ty = local_decl.ty;

        // Create root move path for this local
        let root_idx = arena.push(MovePath {
            place: Place::new(local_idx),
            parent: None,
            children: Vec::new(),
        });

        // Create child move paths for fields of tuple/ADT types
        let ty_ctx = ctx.ty_ctx();
        if let Some(field_count) = count_fields(ty, ty_ctx) {
            for field_idx in 0..field_count {
                let field_proj = ProjectionElem::Field(glyim_type::FieldIdx::from_raw(field_idx));
                let child_place = Place {
                    local: local_idx,
                    projection: Box::new([field_proj]),
                };
                let child_idx = arena.push(MovePath {
                    place: child_place,
                    parent: Some(root_idx),
                    children: Vec::new(),
                });
                arena.get_mut(root_idx).children.push(child_idx);
            }
        }
    }

    arena
}

/// Count the number of fields in a type, if it is a tuple or ADT.
/// Returns None for non-aggregate types (scalars, references, etc.).
fn count_fields(ty: glyim_type::Ty, ctx: &TyCtx) -> Option<u32> {
    match ctx.ty_kind(ty) {
        glyim_type::TyKind::Tuple(substs) => Some(u32::from(substs.len())),
        glyim_type::TyKind::Adt(_, substs) => Some(u32::from(substs.len())),
        glyim_type::TyKind::Array(_, _) | glyim_type::TyKind::Slice(_) => None,
        _ => None,
    }
}

/// Find the move path index for a given place.
fn find_move_path(arena: &MovePathArena, place: &Place) -> Option<MovePathIdx> {
    if place.projection.is_empty() {
        for i in 0..arena.len() {
            let idx = MovePathIdx::from_raw(i as u32);
            let path = arena.get(idx);
            if path.place.local == place.local && path.place.projection.is_empty() {
                return Some(idx);
            }
        }
        return None;
    }

    // For a projected place (e.g., field access), find the child move path
    for i in 0..arena.len() {
        let idx = MovePathIdx::from_raw(i as u32);
        let path = arena.get(idx);
        if path.place.local == place.local
            && path.place.projection.as_ref() == place.projection.as_ref()
        {
            return Some(idx);
        }
    }

    // If no exact match for a projected place, fall back to the root
    if !place.projection.is_empty() {
        for i in 0..arena.len() {
            let idx = MovePathIdx::from_raw(i as u32);
            let path = arena.get(idx);
            if path.place.local == place.local && path.place.projection.is_empty() {
                return Some(idx);
            }
        }
    }

    None
}

/// Find the move path for a local (root path).
fn find_local_move_path(arena: &MovePathArena, local: LocalIdx) -> Option<MovePathIdx> {
    for i in 0..arena.len() {
        let idx = MovePathIdx::from_raw(i as u32);
        let path = arena.get(idx);
        if path.place.local == local && path.place.projection.is_empty() {
            return Some(idx);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Move dataflow analysis
// ---------------------------------------------------------------------------

/// Compute which move paths are moved at each program point using
/// a forward dataflow analysis.
fn compute_move_dataflow(
    body: &Body,
    move_paths: &MovePathArena,
    ctx: &dyn BorrowckCtx,
) -> MoveAnalysisResult {
    let num_blocks = body.basic_blocks.len();
    let num_paths = move_paths.len();

    let mut block_moves: Vec<BitSet> = (0..num_blocks)
        .map(|_| BitSet::with_capacity(num_paths))
        .collect();
    let mut block_inits: Vec<BitSet> = (0..num_blocks)
        .map(|_| BitSet::with_capacity(num_paths))
        .collect();

    for (block_idx, block_data) in body.basic_blocks.iter_enumerated() {
        let bi = block_idx.to_raw() as usize;
        let moves = &mut block_moves[bi];
        let inits = &mut block_inits[bi];

        for stmt in &block_data.statements {
            collect_stmt_move_effects(stmt, move_paths, moves, inits, ctx, &body.locals);
        }
    }

    let mut moved_in: Vec<BitSet> = (0..num_blocks)
        .map(|_| BitSet::with_capacity(num_paths))
        .collect();
    let mut moved_out: Vec<BitSet> = (0..num_blocks)
        .map(|_| BitSet::with_capacity(num_paths))
        .collect();

    let mut predecessors: Vec<Vec<BasicBlockIdx>> = vec![Vec::new(); num_blocks];
    for (block_idx, block_data) in body.basic_blocks.iter_enumerated() {
        for succ in successor_blocks(&block_data.terminator.kind) {
            predecessors[succ.to_raw() as usize].push(block_idx);
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for (block_idx, _block_data) in body.basic_blocks.iter_enumerated() {
            let bi = block_idx.to_raw() as usize;

            let mut new_moved_in = BitSet::with_capacity(num_paths);
            for pred in &predecessors[bi] {
                new_moved_in.union_with(&moved_out[pred.to_raw() as usize]);
            }
            if bi == 0 {
                new_moved_in = BitSet::with_capacity(num_paths);
            }

            let mut new_moved_out = new_moved_in.clone();
            new_moved_out.union_with(&block_moves[bi]);
            new_moved_out.difference_with(&block_inits[bi]);

            if new_moved_in != moved_in[bi] || new_moved_out != moved_out[bi] {
                changed = true;
                moved_in[bi] = new_moved_in;
                moved_out[bi] = new_moved_out;
            }
        }
    }

    let mut local_to_move_path = vec![None; body.locals.len()];
    for i in 0..move_paths.len() {
        let idx = MovePathIdx::from_raw(i as u32);
        let path = move_paths.get(idx);
        if path.place.projection.is_empty() {
            local_to_move_path[path.place.local.to_raw() as usize] = Some(idx);
        }
    }

    MoveAnalysisResult {
        moved_in,
        moved_out,
        move_paths: move_paths.clone_paths(),
        local_to_move_path,
    }
}

/// Collect the move effects of a single statement.
fn collect_stmt_move_effects(
    stmt: &glyim_mir::Statement,
    move_paths: &MovePathArena,
    moves: &mut BitSet,
    inits: &mut BitSet,
    ctx: &dyn BorrowckCtx,
    local_decls: &glyim_core::arena::IndexVec<LocalIdx, LocalDecl>,
) {
    match &stmt.kind {
        StatementKind::Assign(dest, rvalue) => {
            if let Some(mp_idx) = find_move_path(move_paths, dest) {
                inits.insert(mp_idx.to_raw() as usize);
                let path = move_paths.get(mp_idx);
                if path.place.projection.is_empty() {
                    for child in &path.children {
                        inits.insert(child.to_raw() as usize);
                    }
                }
            }

            match rvalue {
                Rvalue::Use(operand) => {
                    collect_operand_move(operand, move_paths, moves, ctx, local_decls);
                }
                Rvalue::Ref(_, _) => {}
                Rvalue::BinaryOp(_, pair) => {
                    let (left, right) = pair.as_ref();
                    collect_operand_move(left, move_paths, moves, ctx, local_decls);
                    collect_operand_move(right, move_paths, moves, ctx, local_decls);
                }
                Rvalue::UnaryOp(_, operand) => {
                    collect_operand_move(operand, move_paths, moves, ctx, local_decls);
                }
                Rvalue::Aggregate(_, operands) => {
                    for op in operands {
                        collect_operand_move(op, move_paths, moves, ctx, local_decls);
                    }
                }
                Rvalue::Discriminant(_) | Rvalue::Len(_) => {}
                Rvalue::Cast(_, operand, _) => {
                    collect_operand_move(operand, move_paths, moves, ctx, local_decls);
                }
                Rvalue::Repeat(operand, _) => {
                    collect_operand_move(operand, move_paths, moves, ctx, local_decls);
                }
            }
        }
        StatementKind::StorageLive(local) => {
            if let Some(mp_idx) = find_local_move_path(move_paths, *local) {
                inits.insert(mp_idx.to_raw() as usize);
                let path = move_paths.get(mp_idx);
                for child in &path.children {
                    inits.insert(child.to_raw() as usize);
                }
            }
        }
        StatementKind::StorageDead(local) => {
            if let Some(mp_idx) = find_local_move_path(move_paths, *local) {
                moves.insert(mp_idx.to_raw() as usize);
                let path = move_paths.get(mp_idx);
                for child in &path.children {
                    moves.insert(child.to_raw() as usize);
                }
            }
        }
        StatementKind::Nop => {}
    }
}

/// Record moves from an operand.
fn collect_operand_move(
    operand: &Operand,
    move_paths: &MovePathArena,
    moves: &mut BitSet,
    ctx: &dyn BorrowckCtx,
    local_decls: &glyim_core::arena::IndexVec<LocalIdx, LocalDecl>,
) {
    match operand {
        Operand::Copy(_) => {}
        Operand::Move(place) => {
            // Check is_copy on the *place type*, not the root local's type.
            // A field of a non-Copy struct may itself be Copy (e.g., i32).
            let place_ty = place.ty(ctx.ty_ctx(), local_decls);
            if !ctx.is_copy(place_ty)
                && let Some(mp_idx) = find_move_path(move_paths, place)
            {
                record_move(move_paths, mp_idx, moves);
            }
        }
        Operand::Constant(_) => {}
    }
}

/// Record a move of a move path and all its children (whole-struct move).
fn record_move(move_paths: &MovePathArena, mp_idx: MovePathIdx, moves: &mut BitSet) {
    moves.insert(mp_idx.to_raw() as usize);
    let path = move_paths.get(mp_idx);
    for child in &path.children {
        moves.insert(child.to_raw() as usize);
    }
}

/// Return the successor basic blocks of a terminator.
fn successor_blocks(kind: &TerminatorKind) -> Vec<BasicBlockIdx> {
    match kind {
        TerminatorKind::Goto { target } => vec![*target],
        TerminatorKind::SwitchInt { targets, .. } => {
            let mut succs: Vec<BasicBlockIdx> = targets.iter().map(|(_, bb)| bb).collect();
            succs.push(targets.otherwise());
            succs
        }
        TerminatorKind::Return | TerminatorKind::Unreachable => vec![],
        TerminatorKind::Call {
            target, cleanup, ..
        } => {
            let mut succs = Vec::new();
            if let Some(t) = target {
                succs.push(*t);
            }
            if let Some(c) = cleanup {
                succs.push(*c);
            }
            succs
        }
        TerminatorKind::Assert {
            target, cleanup, ..
        } => {
            let mut succs = vec![*target];
            if let Some(c) = cleanup {
                succs.push(*c);
            }
            succs
        }
        TerminatorKind::Drop {
            target, cleanup, ..
        } => {
            let mut succs = vec![*target];
            if let Some(c) = cleanup {
                succs.push(*c);
            }
            succs
        }
    }
}

// ---------------------------------------------------------------------------
// Per-statement move state
// ---------------------------------------------------------------------------

fn compute_stmt_moved(
    body: &Body,
    block: BasicBlockIdx,
    moved_in: &BitSet,
    move_paths: &MovePathArena,
    ctx: &dyn BorrowckCtx,
) -> Vec<BitSet> {
    let block_data = &body.basic_blocks[block];
    let num_stmts = block_data.statements.len();
    let num_paths = move_paths.len();

    let mut moved = vec![BitSet::with_capacity(num_paths); num_stmts + 1];
    moved[0] = moved_in.clone();

    for i in 0..num_stmts {
        let mut current = moved[i].clone();
        let stmt = &block_data.statements[i];
        apply_stmt_move_effects(stmt, move_paths, &mut current, ctx, &body.locals);
        moved[i + 1] = current;
    }

    moved
}

fn apply_stmt_move_effects(
    stmt: &glyim_mir::Statement,
    move_paths: &MovePathArena,
    moved: &mut BitSet,
    ctx: &dyn BorrowckCtx,
    local_decls: &glyim_core::arena::IndexVec<LocalIdx, LocalDecl>,
) {
    match &stmt.kind {
        StatementKind::Assign(dest, rvalue) => {
            if let Some(mp_idx) = find_move_path(move_paths, dest) {
                moved.remove(mp_idx.to_raw() as usize);
                let path = move_paths.get(mp_idx);
                if path.place.projection.is_empty() {
                    for child in &path.children {
                        moved.remove(child.to_raw() as usize);
                    }
                }
            }

            match rvalue {
                Rvalue::Use(operand) => {
                    apply_operand_move(operand, move_paths, moved, ctx, local_decls);
                }
                Rvalue::Ref(_, _) => {}
                Rvalue::BinaryOp(_, pair) => {
                    let (left, right) = pair.as_ref();
                    apply_operand_move(left, move_paths, moved, ctx, local_decls);
                    apply_operand_move(right, move_paths, moved, ctx, local_decls);
                }
                Rvalue::UnaryOp(_, operand) => {
                    apply_operand_move(operand, move_paths, moved, ctx, local_decls);
                }
                Rvalue::Aggregate(_, operands) => {
                    for op in operands {
                        apply_operand_move(op, move_paths, moved, ctx, local_decls);
                    }
                }
                Rvalue::Discriminant(_) | Rvalue::Len(_) => {}
                Rvalue::Cast(_, operand, _) => {
                    apply_operand_move(operand, move_paths, moved, ctx, local_decls);
                }
                Rvalue::Repeat(operand, _) => {
                    apply_operand_move(operand, move_paths, moved, ctx, local_decls);
                }
            }
        }
        StatementKind::StorageLive(local) => {
            if let Some(mp_idx) = find_local_move_path(move_paths, *local) {
                moved.remove(mp_idx.to_raw() as usize);
                let path = move_paths.get(mp_idx);
                for child in &path.children {
                    moved.remove(child.to_raw() as usize);
                }
            }
        }
        StatementKind::StorageDead(local) => {
            if let Some(mp_idx) = find_local_move_path(move_paths, *local) {
                record_move(move_paths, mp_idx, moved);
            }
        }
        StatementKind::Nop => {}
    }
}

fn apply_operand_move(
    operand: &Operand,
    move_paths: &MovePathArena,
    moved: &mut BitSet,
    ctx: &dyn BorrowckCtx,
    local_decls: &glyim_core::arena::IndexVec<LocalIdx, LocalDecl>,
) {
    match operand {
        Operand::Copy(_) => {}
        Operand::Move(place) => {
            // Check is_copy on the *place type*, not the root local's type.
            let place_ty = place.ty(ctx.ty_ctx(), local_decls);
            if !ctx.is_copy(place_ty)
                && let Some(mp_idx) = find_move_path(move_paths, place)
            {
                record_move(move_paths, mp_idx, moved);
            }
        }
        Operand::Constant(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Use-after-move checking
// ---------------------------------------------------------------------------

/// Check for use-after-move errors.
pub(crate) fn check_moves(ctx: &dyn BorrowckCtx, body: &Body) -> Vec<GlyimDiagnostic> {
    let mut errors = Vec::new();

    let move_paths = build_move_paths(body, ctx);
    let num_paths = move_paths.len();

    if num_paths == 0 {
        return errors;
    }

    let move_result = compute_move_dataflow(body, &move_paths, ctx);

    for (block_idx, block_data) in body.basic_blocks.iter_enumerated() {
        let block_usize = block_idx.to_raw() as usize;
        let moved_in = &move_result.moved_in[block_usize];
        let stmt_moved = compute_stmt_moved(body, block_idx, moved_in, &move_paths, ctx);

        for (stmt_idx, stmt) in block_data.statements.iter().enumerate() {
            let current_moved = &stmt_moved[stmt_idx];
            check_stmt_use_after_move(stmt, current_moved, &move_paths, &mut errors);
        }
    }

    errors
}

fn check_stmt_use_after_move(
    stmt: &glyim_mir::Statement,
    moved: &BitSet,
    move_paths: &MovePathArena,
    errors: &mut Vec<GlyimDiagnostic>,
) {
    let used_places = collect_used_places(stmt);
    for used_place in used_places {
        check_place_use_after_move(&used_place, stmt, moved, move_paths, errors);
    }
}

fn collect_used_places(stmt: &glyim_mir::Statement) -> Vec<Place> {
    let mut places = Vec::new();
    match &stmt.kind {
        StatementKind::Assign(_, rvalue) => {
            collect_rvalue_used_places(rvalue, &mut places);
        }
        StatementKind::StorageLive(_) | StatementKind::StorageDead(_) | StatementKind::Nop => {}
    }
    places
}

fn collect_rvalue_used_places(rvalue: &Rvalue, places: &mut Vec<Place>) {
    match rvalue {
        Rvalue::Use(operand) => {
            collect_operand_used_places(operand, places);
        }
        Rvalue::Ref(place, _) => {
            places.push(place.clone());
        }
        Rvalue::BinaryOp(_, pair) => {
            let (left, right) = pair.as_ref();
            collect_operand_used_places(left, places);
            collect_operand_used_places(right, places);
        }
        Rvalue::UnaryOp(_, operand) => {
            collect_operand_used_places(operand, places);
        }
        Rvalue::Aggregate(_, operands) => {
            for op in operands {
                collect_operand_used_places(op, places);
            }
        }
        Rvalue::Discriminant(place) | Rvalue::Len(place) => {
            places.push(place.clone());
        }
        Rvalue::Cast(_, operand, _) => {
            collect_operand_used_places(operand, places);
        }
        Rvalue::Repeat(operand, _) => {
            collect_operand_used_places(operand, places);
        }
    }
}

fn collect_operand_used_places(operand: &Operand, places: &mut Vec<Place>) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            places.push(place.clone());
        }
        Operand::Constant(_) => {}
    }
}

fn check_place_use_after_move(
    place: &Place,
    stmt: &glyim_mir::Statement,
    moved: &BitSet,
    move_paths: &MovePathArena,
    errors: &mut Vec<GlyimDiagnostic>,
) {
    // Try to find the exact move path for this place
    if let Some(mp_idx) = find_move_path(move_paths, place)
        && moved.contains(mp_idx.to_raw() as usize)
    {
        let msg = if place.projection.is_empty() {
            format!("use of moved value: `_{}`", place.local.to_raw())
        } else {
            format!(
                "use of moved value: `_{}.{}...`",
                place.local.to_raw(),
                describe_projection(&place.projection)
            )
        };
        errors.push(GlyimDiagnostic::borrow_error(stmt.source_info.span, msg));
        return;
    }

    // For a bare local, also check if any of its fields have been moved
    if place.projection.is_empty()
        && let Some(root_idx) = find_local_move_path(move_paths, place.local)
    {
        let root_path = move_paths.get(root_idx);
        let any_child_moved = root_path
            .children
            .iter()
            .any(|child| moved.contains(child.to_raw() as usize));
        if any_child_moved {
            let msg = format!("use of partially moved value: `_{}`", place.local.to_raw());
            errors.push(GlyimDiagnostic::borrow_error(stmt.source_info.span, msg));
        }
    }
}

fn describe_projection(projection: &[ProjectionElem]) -> String {
    projection
        .iter()
        .map(|elem| match elem {
            ProjectionElem::Deref => "deref".to_string(),
            ProjectionElem::Field(idx) => format!("field{}", idx.to_raw()),
            ProjectionElem::Index(_) => "index".to_string(),
            ProjectionElem::Downcast(idx) => format!("variant{}", idx.to_raw()),
        })
        .collect::<Vec<_>>()
        .join(".")
}
