You are implementing Stream U-CLI: Unstub CLI for the Glyim compiler.

## Mission
Remove all stubs in `glyim-cli` related to emit modes and linker invocation. Fix duplicate clap attribute bug. Implement --emit mir, --emit llvm-ir, --emit exec. Create linker.rs that shells out to cc to link .o against libc.

## What You Own Exclusively (DO NOT touch any other files)
- `crates/glyim-cli/src/lib.rs`
- `crates/glyim-cli/src/main.rs`
- `crates/glyim-cli/src/linker.rs` (NEW FILE)

## Exact Implementation Guide (NO STUBS ALLOWED)

### 1. Fix CLI Args (`lib.rs`)
Remove the dangling duplicate `#[arg(...)]` attribute above `opt_level`.
Replace:
```rust
#[arg(long, value_name = "EMIT", default_value = "obj")]
pub emit: String,
#[arg(long, value_name = "EMIT", default_value = "obj")]
#[arg(short = 'O', long = "opt-level", default_value = "0")]
pub opt_level: u8,
```
With:
```rust
#[arg(long, value_name = "EMIT", default_value = "obj")]
pub emit: String,
#[arg(short = 'O', long = "opt-level", default_value = "0")]
pub opt_level: u8,
```

### 2. Implement Emit Logic (`lib.rs`)
In `run_with_args`, read `args.emit` and branch:
```rust
match args.emit.as_str() {
    "mir" => {
        Pipeline::emit_mir(&mut db, &args.input)?;
        return Ok(());
    }
    "llvm-ir" => {
        Pipeline::emit_llvm_ir(&mut db, &args.input)?;
        return Ok(());
    }
    "obj" | "exec" => { /* continue to backend */ }
    _ => return Err(vec![GlyimDiagnostic::internal_error("unknown emit type")])
}
```

### 3. Create Linker (`linker.rs`)
Implement the linker invocation using `cc`:
```rust
pub fn invoke_linker(obj_path: &std::path::Path, output_path: &std::path::Path) -> Result<(), String> {
    let status = std::process::Command::new("cc")
        .arg(obj_path)
        .arg("-o").arg(output_path)
        .status()
        .map_err(|e| format!("Failed to invoke linker: {}", e))?;
    if status.success() { Ok(()) } else { Err("Linker failed".to_string()) }
}
```

### 4. Wire Linker (`lib.rs` or `main.rs`)
After `Pipeline::compile_file` generates the `.o` file, if `args.emit == "exec"`, call `linker::invoke_linker(&output_path, &final_exec_path)`.

## Execution Rules (MANDATORY: plan-to-cat-scripts skill)
You MUST follow the `plan-to-cat-scripts` skill exactly. Output ONLY fenced bash code blocks.

1. **Setup:** First script MUST set `STREAM_ID="U-CLI"`, `WORKTREE_DIR="../glyim-worktrees/stream-U-CLI"`. Use `git worktree add --detach "$WORKTREE_DIR" main`, cd into it, and `git checkout -b "stream-${STREAM_ID}/v0.1.0"`.
2. **No `#` comments:** Every action must be logged with `echo`.
3. **Heredocs:** MUST use the fixed delimiter `EOF`. Ensure no lines in the content are exactly `EOF`.
4. **Patches:** For trivial single-line replacements use `sed`. For multi-line replacements, use Python with temp files (heredocs with `EOF`). No Python string literals containing the content.
5. **Tests:** Do not create a tests directory for this crate; just ensure the code compiles.
6. **Verify:** Run `cargo check --workspace` at the end. If `COMPILE_OK=true`, run tests and commit with `stream-U-CLI: feat(cli): unstub emit modes and add linker`.
