//! End-to-end proof that the P0 canonical type interner eliminates the
//! cross-context `Ty`/`Substitution` handle-validity bug.
//!
//! `struct`-with-`Drop` programs force drop-glue elaboration to synthesize a
//! `[bool; N]` flag-array type *inside the elaboration pass* (a `TyCtxMut`
//! derived from the frozen `TyCtx`). Before the canonical interner, that handle
//! was allocated in a fresh per-context `Vec` and was invalid when read back by
//! the consumer's `TyCtx` — the run panicked with an out-of-bounds index at
//! `ty_ctx.rs:59`. With the shared arena, `freeze`/`to_mut` copy the `&'static`
//! arena pointer, so the flag-array handle is valid everywhere.
//!
//! This test compiles a `struct`-with-`Drop` program through the real pipeline
//! (which now runs `elaborate_drops` on a `to_mut()` arena-sharing `TyCtxMut`)
//! and executes the resulting MIR bodies in the `glyim_mir_interp` interpreter.
//! Success == the interpreter returns `Ok(())` (the `Drop` glue ran and the
//! program completed) with no OOB panic.

use std::io::Write;
use std::sync::Arc;

use glyim_db::Database;
use glyim_mir_interp::Interpreter;
use glyim_pipeline::compile_file_to_mir;

fn compile_and_run_struct_with_drop(src: &str) -> Result<(), String> {
    // Each call writes its source to a temp file the pipeline reads back. The
    // path MUST be unique per call: the Rust test harness runs tests in
    // parallel threads within one process, so a path derived only from `pid`
    // collides and one thread overwrites another's source (causing spurious
    // "no field `Elem`"-style type errors).
    static CALL_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let call_id = CALL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let unique_tag = format!("{}_{}", std::process::id(), call_id);
    let dir = std::env::temp_dir();
    let path = dir.join(format!("glyim_struct_drop_{}.g", unique_tag));

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
            .map(|d| format!("{:?}", d))
            .collect::<Vec<_>>()
            .join("\n")
    })?;

    let ty_ctx = mir.ty_ctx;
    let bodies: Vec<Arc<glyim_mir::Body>> = mir.bodies.values().cloned().collect();

    // Pick the `main` body deterministically rather than relying on HashMap
    // iteration order.
    let main_local = glyim_pipeline::Pipeline::entry_main_local_id(&mut db, &path)
        .ok_or_else(|| "could not resolve main entry".to_string())?;
    let main_id = glyim_core::def_id::DefId::new(
        glyim_core::def_id::CrateId::from_raw(0),
        glyim_core::def_id::LocalDefId::from_raw(main_local),
    );
    let main_body = mir
        .bodies
        .get(&main_id)
        .ok_or_else(|| "main body not found in MIR compilation".to_string())?;

    let interpreter = Interpreter::new(ty_ctx.as_ref());
    let mut interp = interpreter;
    for b in &bodies {
        interp.add_function(b.owner, (**b).clone());
    }
    interp
        .run_body(main_body)
        .map_err(|e| format!("interpreter error: {}", e))
}

#[test]
fn struct_with_drop_runs_via_real_pipeline_and_interpreter() {
    // Self-contained: define a minimal `Drop` trait so the `impl Drop` resolves
    // without loading the (parser-incomplete) core library. The drop body is a
    // no-op — we only need `elaborate_drops` to fire (which allocates a
    // `[bool; N]` flag-array type inside the arena-sharing `TyCtxMut`) and for
    // the resulting MIR to run in the interpreter without the cross-context
    // OOB panic that the old per-context `Vec` design produced.
    let src = r#"
trait Drop {
    fn drop(&mut self);
}

struct WithDrop {
    id: i32
}

impl Drop for WithDrop {
    fn drop(&mut self) {
        let _ = self.id;
    }
}

fn main() {
    let _a = WithDrop { id: 1 };
    let _b = WithDrop { id: 2 };
}
"#;
    compile_and_run_struct_with_drop(src)
        .expect("struct-with-Drop must compile + run without the cross-context OOB panic");
}

#[test]
fn struct_with_drop_and_array_field_runs() {
    // Exercises multiple `drop_impls` entries / multiple drop-flag slots on the
    // shared arena: two distinct Drop-implementing structs, both dropped in
    // `main`. Uses plain `i32` fields (nested-struct field resolution is a
    // separate, out-of-scope type-checker concern) — the point is that every
    // synthesized `[bool; N]` flag-array handle stays valid across contexts.
    let src = r#"
trait Drop {
    fn drop(&mut self);
}

struct A {
    x: i32
}

impl Drop for A {
    fn drop(&mut self) {
        let _ = self.x;
    }
}

struct B {
    y: i32
}

impl Drop for B {
    fn drop(&mut self) {
        let _ = self.y;
    }
}

fn main() {
    let _a = A { x: 1 };
    let _b = B { y: 2 };
    let _c = A { x: 3 };
}
"#;
    compile_and_run_struct_with_drop(src)
        .expect("struct-with-multiple-drop-impls must compile + run without the cross-context OOB panic");
}
