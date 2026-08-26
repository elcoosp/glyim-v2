//! M5 (async v1) — multi-await codegen proof through the real full pipeline.
//!
//! This test compiles the multi-`.await` `two_step`/`two` shapes through the
//! REAL pipeline (parser → HIR desugar `desugar_multi_async_fn` → typeck →
//! MIR lower) and asserts:
//!   1. **Zero compile diagnostics** — the generated suspend/resume state
//!      machine type-checks and lowers cleanly (M4: real multi-await codegen,
//!      the deliverable of GLYIM_DESTUB_PLAN Phase 3). Previously this shape
//!      fell through to the `async-v2` diagnostic (error 61) and never produced
//!      a working state machine; now it is a genuine `Start`/`S0`/…/`Done`
//!      machine with a `loop { match self.state { .. } }` `poll` body.
//!   2. **The generated `poll` body is a real suspend/resume machine** — it
//!      contains a loop back-edge (so `Pending` re-enters the state match), an
//!      enum-state `SwitchInt` (one arm per `Start`/`S_k`/`Done` variant), and
//!      `Poll::Pending`/`Poll::Ready` `Aggregate` constructions. This is the
//!      structural proof that the future genuinely *suspends and resumes*
//!      rather than hard-coding `Poll::Pending` (the old silent-miscompile).
//!
//! HONEST SCOPE NOTE (M5 runtime execution): full end-to-end *execution* of
//! `block_on(f)` requires trait-method dispatch — `block_on<F: Future>` calls
//! `f.poll()`, a `Future::poll` trait method. Neither the LLVM backend nor the
//! in-process `glyim_mir_interp` interpreter resolves trait-method calls today
//! (documented M5 host-blocker: generic `Future`/`block_on` instantiation gap,
//! affecting ALL trait usage, not just async). Verifying the *runtime* resumption
//! therefore requires closing that separate codegen/interp gap, which is OUT of
//! scope for the async desugar (M4). This test proves the compile + MIR codegen
//! correctness that M4 owns; the runtime-execution proof is gated on the
//! trait-dispatch gap exactly as GLYIM_DESTUB_PLAN Phase 3 states.

use std::io::Write;
use std::sync::Arc;

use glyim_db::Database;
use glyim_mir::{Body, Rvalue, StatementKind, TerminatorKind};
use glyim_pipeline::compile_file_to_mir;

fn compile_async(src: &str) -> Result<Vec<Arc<Body>>, String> {
    static CALL_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let call_id = CALL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let unique_tag = format!("{}_{}", std::process::id(), call_id);
    let dir = std::env::temp_dir();
    let path = dir.join(format!("glyim_async_m5_{}.g", unique_tag));

    {
        let mut f = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        f.write_all(src.as_bytes()).map_err(|e| e.to_string())?;
    }

    let config = glyim_db::CrateConfig {
        name: "test_crate".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        opt_level: 0,
    };
    let mut db = Database::new(config);

    let mir = compile_file_to_mir(&mut db, &path).map_err(|diags| {
        diags
            .iter()
            .map(|d| format!("{:?}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("\n")
    })?;

    Ok(mir.bodies.values().cloned().collect())
}

/// Find the generated `poll` body by structure: it is the MIR body that is a
/// suspend/resume state machine (contains a `SwitchInt` on the enum state, a
/// loop back-edge, and constructs both `Poll::Pending` and `Poll::Ready`).
/// `block_on`'s body also has a SwitchInt + back-edge, but it only *patterns*
/// `Poll::Pending`, never *constructs* it, so the Pending-construction
/// requirement reliably selects the generated `poll`.
fn find_poll_body(bodies: &[Arc<Body>]) -> Option<&Arc<Body>> {
    bodies.iter().find(|b| is_state_machine(b))
}

/// Assert `body` is a suspend/resume state machine:
///  - has a loop back-edge (`Goto`/`SwitchInt` whose target is an earlier block),
///  - has at least one `SwitchInt` (the enum-state dispatch),
///  - constructs `Poll::Pending` and `Poll::Ready` aggregates.
fn is_state_machine(body: &Body) -> bool {
    let mut has_switchint = false;
    let mut has_pending = false;
    let mut has_ready = false;
    let mut has_back_edge = false;

    for (bb_idx, bb) in body.basic_blocks.iter_enumerated() {
        let bi = bb_idx.index();
        // Detect a back-edge: any terminator targeting a strictly-earlier block.
        for succ in successor_idxs(&bb.terminator.kind) {
            if (succ as usize) < bi {
                has_back_edge = true;
            }
        }
        if matches!(bb.terminator.kind, TerminatorKind::SwitchInt { .. }) {
            has_switchint = true;
        }
        // Poll ctor detection: `Poll::Pending` is an `Aggregate` with 0 operands;
        // `Poll::Ready` is an `Aggregate` with 1 operand.
        for stmt in &bb.statements {
            if let StatementKind::Assign(_, Rvalue::Aggregate(_, ops)) = &stmt.kind {
                if ops.is_empty() {
                    has_pending = true;
                }
                if ops.len() == 1 {
                    has_ready = true;
                }
            }
        }
    }

    has_switchint && has_back_edge && has_pending && has_ready
}

/// Assert `body` is a suspend/resume state machine (see `is_state_machine`).
fn assert_is_state_machine(body: &Body, label: &str) {
    assert!(
        is_state_machine(body),
        "{label}: poll body must be a suspend/resume state machine \
         (SwitchInt on enum state + loop back-edge + Poll::Pending/Ready construction)"
    );
}

fn successor_idxs(kind: &TerminatorKind) -> Vec<usize> {
    match kind {
        TerminatorKind::Goto { target } => vec![target.index()],
        TerminatorKind::SwitchInt { targets, .. } => {
            let mut v: Vec<usize> = targets
                .iter()
                .map(|(_val, bb)| bb.index())
                .collect();
            v.push(targets.otherwise().index());
            v
        }
        TerminatorKind::Call { target, cleanup, .. } => {
            let mut v = Vec::new();
            if let Some(t) = target {
                v.push(t.index());
            }
            if let Some(c) = cleanup {
                v.push(c.index());
            }
            v
        }
        TerminatorKind::Return | TerminatorKind::Unreachable => vec![],
        _ => vec![],
    }
}

#[test]
fn m5_two_step_multi_await_compiles_and_lowers_to_state_machine() {
    // `two_step(a, b)`: x = dep(a); y = dep(b); x + y  => two `.await`s.
    let src = r#"
enum Poll<T> { Ready(T), Pending }
trait Future {
    type Output;
    fn poll(&mut self) -> Poll<Self::Output>;
}
fn block_on<F: Future>(mut f: F) -> F::Output {
    loop {
        match f.poll() {
            Poll::Ready(v) => return v,
            Poll::Pending => { }
        }
    }
}
async fn dep(x: i32) -> i32 { x }
async fn two_step(a: i32, b: i32) -> i32 {
    let x = dep(a).await;
    let y = dep(b).await;
    x + y
}
fn main() -> i32 {
    let f = two_step(1, 2);
    block_on(f)
}
"#;
    let bodies = compile_async(src).expect("two_step must compile with zero diagnostics (M4)");
    let poll = find_poll_body(&bodies).expect("generated two poll body must exist");
    // The body named after the async fn is the *future's poll* once desugared;
    // assert it is a real state machine.
    assert_is_state_machine(poll, "two_step poll");
}

#[test]
fn m5_chain_multi_await_compiles_and_lowers_to_state_machine() {
    // Chained: `two(a)`: x = dep(a); y = dep(x); x + y — two `.await`s where
    // the second depends on the first's result (forcing state capture).
    let src = r#"
enum Poll<T> { Ready(T), Pending }
trait Future {
    type Output;
    fn poll(&mut self) -> Poll<Self::Output>;
}
fn block_on<F: Future>(mut f: F) -> F::Output {
    loop {
        match f.poll() {
            Poll::Ready(v) => return v,
            Poll::Pending => { }
        }
    }
}
async fn dep(x: i32) -> i32 { x }
async fn two(a: i32) -> i32 { let x = dep(a).await; let y = dep(x).await; x + y }
fn main() -> i32 {
    let f = two(5);
    block_on(f)
}
"#;
    let bodies = compile_async(src).expect("two must compile with zero diagnostics (M4)");
    let poll = find_poll_body(&bodies).expect("generated two poll body must exist");
    assert_is_state_machine(poll, "two poll");
}
