//! Backward dataflow liveness analysis for MIR locals.
//!
//! A local is *live* at a program point if its current value may be used on
//! some execution path starting from that point before being overwritten.
//!
//! The analysis uses a worklist-driven fixed-point iteration that only
//! reprocesses blocks whose inputs have changed, which is significantly
//! faster than the naïve "iterate everything until stable" approach.

use fixedbitset::FixedBitSet as BitSet;
use glyim_mir::{BasicBlockIdx, Body, StatementKind, TerminatorKind};
use tracing::trace;

use crate::visitor::{
    LivenessGen, LivenessUseCollector, successor_blocks, walk_rvalue_reads, walk_terminator_reads,
};

// ---------------------------------------------------------------------------
// Public result type
// ---------------------------------------------------------------------------

/// Result of the backward dataflow liveness analysis.
pub(crate) struct LivenessResult {
    /// For each basic block, the set of locals live on entry.
    #[allow(dead_code)]
    pub live_in: Vec<BitSet>,
    /// For each basic block, the set of locals live on exit.
    pub live_out: Vec<BitSet>,
}

// ---------------------------------------------------------------------------
// Block-level use / def computation
// ---------------------------------------------------------------------------

/// Per-block summary of which locals are used before being defined ("upward
/// exposed uses") and which locals are defined.
struct BlockSummary {
    uses: Vec<BitSet>,
    defs: Vec<BitSet>,
}

/// Compute per-block use/def summaries.
///
/// **Correctness note:** within a single `Assign` statement like `x = x + 1`,
/// the rvalue is evaluated *before* the lvalue is written. We therefore
/// collect rvalue uses **first**, then record the lvalue def. Recording the
/// def first (the previous buggy order) caused the def to mask the use in
/// statements where the same local appears on both sides.
fn compute_block_summaries(body: &Body) -> BlockSummary {
    let num_blocks = body.basic_blocks.len();
    let num_locals = body.locals.len();

    let mut uses: Vec<BitSet> = (0..num_blocks)
        .map(|_| BitSet::with_capacity(num_locals))
        .collect();
    let mut defs: Vec<BitSet> = (0..num_blocks)
        .map(|_| BitSet::with_capacity(num_locals))
        .collect();

    for (block_idx, block_data) in body.basic_blocks.iter_enumerated() {
        let bi = block_idx.to_raw() as usize;
        let bu = &mut uses[bi];
        let bd = &mut defs[bi];

        for stmt in &block_data.statements {
            if let StatementKind::Assign(place, rvalue) = &stmt.kind {
                // 1. Collect rvalue uses FIRST (rvalue is evaluated before lvalue write)
                {
                    let mut collector = LivenessUseCollector { uses: bu, defs: bd };
                    walk_rvalue_reads(rvalue, &mut collector);
                }
                // 2. THEN record the lvalue def
                bd.insert(place.local.to_raw() as usize);
            }
        }

        // Terminator reads come after all statement defs.
        {
            let mut collector = LivenessUseCollector { uses: bu, defs: bd };
            walk_terminator_reads(&block_data.terminator.kind, &mut collector);
        }

        // Terminator defs (e.g. Call destination)
        if let TerminatorKind::Call { destination, .. } = &block_data.terminator.kind {
            defs[bi].insert(destination.local.to_raw() as usize);
        }
    }

    BlockSummary { uses, defs }
}

// ---------------------------------------------------------------------------
// Worklist-based fixed-point iteration
// ---------------------------------------------------------------------------

/// Compute liveness of all locals using a worklist-driven backward dataflow
/// analysis on the CFG.
///
/// **Complexity:** O(|blocks| × |locals| / word_size) per reprocessing, but
/// the worklist ensures each block is only reprocessed when its successors'
/// `live_in` sets actually change.
pub(crate) fn compute_liveness(body: &Body) -> LivenessResult {
    let num_blocks = body.basic_blocks.len();
    let num_locals = body.locals.len();

    let summary = compute_block_summaries(body);

    // Precompute successor and predecessor lists (avoid recomputing per iteration)
    let mut successors: Vec<Vec<BasicBlockIdx>> = Vec::with_capacity(num_blocks);
    let mut predecessors: Vec<Vec<BasicBlockIdx>> = vec![Vec::new(); num_blocks];
    for (block_idx, block_data) in body.basic_blocks.iter_enumerated() {
        let sucs = successor_blocks(&block_data.terminator.kind);
        for &succ in &sucs {
            predecessors[succ.to_raw() as usize].push(block_idx);
        }
        successors.push(sucs);
    }

    let mut live_in: Vec<BitSet> = (0..num_blocks)
        .map(|_| BitSet::with_capacity(num_locals))
        .collect();
    let mut live_out: Vec<BitSet> = (0..num_blocks)
        .map(|_| BitSet::with_capacity(num_locals))
        .collect();

    // Initialise worklist with every block
    let mut worklist: Vec<usize> = (0..num_blocks).collect();
    let mut in_worklist = BitSet::with_capacity(num_blocks);
    for i in 0..num_blocks {
        in_worklist.insert(i);
    }

    while let Some(bi) = worklist.pop() {
        in_worklist.remove(bi);

        // live_out(B) = ∪ live_in(S) for all successors S
        let mut new_live_out = BitSet::with_capacity(num_locals);
        for &succ in &successors[bi] {
            new_live_out.union_with(&live_in[succ.to_raw() as usize]);
        }

        // live_in(B) = uses(B) ∪ (live_out(B) − defs(B))
        let mut new_live_in = summary.uses[bi].clone();
        {
            let mut diff = new_live_out.clone();
            diff.difference_with(&summary.defs[bi]);
            new_live_in.union_with(&diff);
        }

        if new_live_in != live_in[bi] || new_live_out != live_out[bi] {
            trace!(block = bi, "liveness changed — enqueuing predecessors");
            live_in[bi] = new_live_in;
            live_out[bi] = new_live_out;

            for &pred in &predecessors[bi] {
                let pi = pred.to_raw() as usize;
                if !in_worklist.contains(pi) {
                    in_worklist.insert(pi);
                    worklist.push(pi);
                }
            }
        }
    }

    trace!("liveness analysis complete");
    LivenessResult { live_in, live_out }
}

// ---------------------------------------------------------------------------
// Per-statement liveness within a single basic block
// ---------------------------------------------------------------------------

/// Compute per-statement liveness within a basic block by scanning backward
/// from `live_out`.
///
/// Returns a vector where `result[i]` is the set of locals live just
/// **before** statement `i` executes. `result[num_stmts]` equals `live_out`
/// (live just after the last statement, before the terminator).
pub(crate) fn compute_stmt_liveness(
    body: &Body,
    block: BasicBlockIdx,
    live_out: &BitSet,
) -> Vec<BitSet> {
    let block_data = &body.basic_blocks[block];
    let num_stmts = block_data.statements.len();
    let num_locals = body.locals.len();

    let mut liveness = vec![BitSet::with_capacity(num_locals); num_stmts + 1];
    liveness[num_stmts] = live_out.clone();

    for i in (0..num_stmts).rev() {
        let mut current = liveness[i + 1].clone();
        let stmt = &block_data.statements[i];
        if let StatementKind::Assign(place, rvalue) = &stmt.kind {
            // Kill: the destination is defined here.
            current.remove(place.local.to_raw() as usize);
            // Gen: all locals read by the rvalue.
            {
                // Note: `gen` is a reserved keyword in edition 2024
                let mut liveness_gen = LivenessGen { live: &mut current };
                walk_rvalue_reads(rvalue, &mut liveness_gen);
            }
        }
        liveness[i] = current;
    }

    liveness
}
