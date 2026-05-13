# Glyim Test Framework — Agent Instructions

## Mandatory: Test Location

**ALL tests MUST be written as unit tests inside `crates/*/src/tests/`, NOT as integration tests in `crates/*/tests/`.**

This keeps compilation fast because unit tests share the crate's private API and compile in the same crate unit. Integration tests compile as separate crate targets, which is significantly slower.

### Directory Structure

```
crates/glyim-type/
├── src/
│   ├── lib.rs          ← add: mod tests;
│   ├── context.rs
│   └── tests/          ← YOUR TESTS GO HERE
│       ├── mod.rs      ← add: mod interning; mod freeze; mod substitution;
│       ├── interning.rs
│       ├── freeze.rs
│       └── substitution.rs
└── Cargo.toml
```

### How to Wire It Up

In `crates/glyim-type/src/lib.rs`, add:

```rust
#[cfg(test)]
mod tests;
```

In `crates/glyim-type/src/tests/mod.rs`, declare submodules:

```rust
mod interning;
mod freeze;
mod substitution;
```

Each test file is a normal Rust file with `#[test]` functions. They have full access to `pub(crate)` items.

---

## Using `glyim-test` Crate

`glyim-test` is a workspace crate that provides testing utilities, mocks, assertions, and fixtures. Add it as a dev-dependency:

```toml
[dev-dependencies]
glyim-test = { workspace = true }
```

Then use it in your test files:

```rust
// crates/glyim-type/src/tests/interning.rs

use glyim_test::{test_ty_ctx, test_frozen_ty_ctx, with_fresh_ty_ctx, assert_ty, check_ty};
use glyim_test::fixtures::TyFactory;
use glyim_core::primitives::*;
use glyim_type::*;

#[test]
fn sentinel_error() {
    let ctx = test_frozen_ty_ctx();
    assert!(matches!(ctx.ty_kind(Ty::ERROR), TyKind::Error));
}

#[test]
fn mk_ref_roundtrip() {
    let (ctx, ref_ty) = with_fresh_ty_ctx(|ctx_mut| {
        let inner = ctx_mut.bool_ty();
        ctx_mut.mk_ref(Region::Erased, inner, Mutability::Mut)
    });
    assert_ty(&ctx, ref_ty)
        .is_ref(Mutability::Mut)
        .is_bool();
}

#[test]
fn check_ty_composable() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    let result = check_ty(&ctx, ty).is_bool().is_not_error().finish();
    assert!(result.is_ok());
}
```

---

## Core APIs Reference

### Type Context Helpers

| Function | Signature | Purpose |
|----------|-----------|---------|
| `test_ty_ctx()` | `-> TyCtxMut` | Create a fresh mutable type context |
| `test_frozen_ty_ctx()` | `-> TyCtx` | Create a frozen (Send+Sync) type context |
| `with_fresh_ty_ctx(f)` | `(TyCtx, R)` | Run function with TyCtxMut, freeze, return both |

### Type Construction (TyFactory)

Use `TyFactory` or `TyCtxMut` methods. **NEVER call `Ty::from_raw()` or `Substitution::from_raw()`** — these are `pub(crate)`.

| Method | Returns |
|--------|---------|
| `ctx.bool_ty()` | `Ty::BOOL` |
| `ctx.never_ty()` | `Ty::NEVER` |
| `ctx.unit_ty()` | `Ty::UNIT` |
| `ctx.mk_ty(TyKind::Int(IntTy::I32))` | `i32` type |
| `ctx.mk_ref(Region::Erased, inner, Mutability::Not)` | `&T` |
| `ctx.intern_substitution(vec![GenericArg::Ty(ty)])` | `Substitution` |
| `TyFactory::i32(ctx)` | `i32` type |
| `TyFactory::ref_to(ctx, inner, Mutability::Mut)` | `&mut T` |

### Type Assertions

Two APIs: **panic-based** (`TyAssert`) and **Result-based** (`TyCheck`).

```rust
// Panic-based: stops on first failure
assert_ty(&ctx, ty).is_int(IntTy::I32);
assert_ty(&ctx, ty).is_ref(Mutability::Mut).is_bool();
assert_ty(&ctx, ty).is_error();
assert_ty(&ctx, ty).is_never();
assert_ty(&ctx, ty).is_unit();
assert_ty(&ctx, ty).has_infer();
assert_ty(&ctx, ty).has_no_infer();

// Result-based: collects all failures
let result = check_ty(&ctx, ty)
    .is_int(IntTy::I32)
    .is_not_error()
    .has_no_infer()
    .finish();
assert!(result.is_ok());
```

### MIR Assertions

```rust
use glyim_test::assert_mir;

assert_mir(&ctx, &body)
    .block_count(3)
    .local_count(5)
    .block_terminator(0, "Goto")
    .block_terminator(1, "SwitchInt")
    .block_terminator(2, "Return");
```

### Diagnostic Assertions

```rust
use glyim_test::{assert_no_errors, assert_has_errors, assert_error_count, assert_diag_contains};

assert_no_errors(&diagnostics);
assert_has_errors(&diagnostics);
assert_error_count(&diagnostics, 2);
assert_diag_contains(&diagnostics, "mismatched types");
```

### Layout Assertions

```rust
use glyim_test::assert_layout;

assert_layout(&ctx, bool_ty, 1, 1);  // size=1, align=1
assert_layout(&ctx, i32_ty, 4, 4);   // size=4, align=4
```

### Snapshot Testing

```rust
use glyim_test::{snapshot_cst, snapshot_mir, snapshot_def_map};

snapshot_cst("parse_fn", "fn main() {}");
snapshot_mir("lower_add", &ctx, &body);
snapshot_def_map("simple_module", &def_map);
```

---

## Mock Implementations

All mocks implement their **real upstream traits**. Use them when your stream depends on a crate that isn't implemented yet.

### MockSolver (implements `glyim_solve::TraitSolver`)

```rust
use glyim_test::mock::MockSolver;
use glyim_solve::TraitSolver;

let mut solver = MockSolver::new()
    .respond_for_any(glyim_solve::SolverResult::Proven);

// After calling solver.can_prove(...)
assert_eq!(solver.call_count(), 1);
```

### MockCodegen (implements `glyim_codegen::CodegenBackend`)

```rust
use glyim_test::mock::MockCodegen;
use glyim_codegen::CodegenBackend;

let mock = MockCodegen::new();
assert_eq!(mock.name(), "mock");

mock.generate(&bodies, &output_path).unwrap();
```

### MockLowerCtx (implements `glyim_lower::LowerCtx`)

```rust
use glyim_test::mock::MockLowerCtx;
use glyim_lower::LowerCtx;

let ctx = test_frozen_ty_ctx();
let mock = MockLowerCtx::new(&ctx);
mock.push_span(Span::DUMMY);
mock.pop_span();
mock.assert_spans_balanced();
```

### MockBorrowckCtx (implements `glyim_borrowck::BorrowckCtx`)

```rust
use glyim_test::mock::MockBorrowckCtx;
use glyim_borrowck::BorrowckCtx;

let mock = MockBorrowckCtx::new(&ctx, &body);
```

### TestDbBuilder

```rust
use glyim_test::mock::TestDbBuilder;
use std::sync::Arc;

let db = TestDbBuilder::new()
    .name("my_test")
    .target_triple("x86_64-unknown-linux-gnu")
    .opt_level(0)
    .file(PathBuf::from("main.g"), Arc::from("fn main() {}"))
    .build();
```

---

## Property-Based Testing

### Concrete Types Only

```rust
use glyim_test::check_ty_property;

let result = check_ty_property(42, 100, |ctx, ty| {
    if ctx.ty_is_error(ty) { return Ok(()); }
    Ok(())
});
assert!(result.is_ok());
```

### Unification Testing

```rust
use glyim_test::property::unify;
use glyim_solve::InferenceTable;

let mut ctx = test_ty_ctx();
let mut infer = InferenceTable::new();

let var = infer.new_ty_var(&mut ctx);
let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)));
let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));

unify::test_unify_var_with_concrete(&mut ctx, &mut infer, var_ty, i32_ty);
```

---

## Phase Testing

### Frontend Only (Lex + Parse)

```rust
use glyim_test::phase::FrontendTester;

let trace = FrontendTester::new("fn main() {}").run();
assert!(trace.parse_tree.is_some());
assert_no_errors(&trace.parse_diagnostics);
```

### Full Pipeline

```rust
use glyim_test::harness::compiler::PipelineCompiler;
use glyim_test::mock::MockCodegen;
use glyim_codegen::CodegenBackend;
use std::sync::Arc;

let backend = Arc::new(MockCodegen::new());
let compiler = PipelineCompiler::new(backend);
let output = compiler.compile("fn main() {}", FileId::from_raw(1), &[]);
assert_no_errors(&output.diagnostics);
```

---

## Harness: Running File-Based Tests

For streams that need file-based testing (e.g., lexer, parser, typeck), use the test harness:

```rust
// In your crate's src/tests/harness_tests.rs
use glyim_test::harness::{TestRunner, TestMode};

#[test]
fn compile_pass_tests() {
    TestRunner::new("tests/compile-pass")
        .mode(TestMode::CompilePass)
        .parallel(true)
        .build()
        .expect("failed to discover tests")
        .run();
}

#[test]
fn compile_fail_tests() {
    TestRunner::new("tests/compile-fail")
        .mode(TestMode::CompileFail)
        .build()
        .expect("failed to discover tests")
        .run();
}

#[test]
fn ui_tests() {
    TestRunner::new("tests/ui")
        .mode(TestMode::Ui)
        .build()
        .expect("failed to discover tests")
        .run();
}
```

Test files use annotations:

```gly
// test-mode: compile-fail
// error-pattern: mismatched types

fn main() {
    let x: i32 = "hello"; //~ ERROR mismatched types
    let y: bool = 42;       //~ ERROR mismatched types
                           //~| NOTE expected `bool`
}
```

Annotation syntax:
- `//~ ERROR msg` — exact line match
- `//~~ ERROR msg` — fuzzy match (±1 line)
- `//~? ERROR msg` — optional (no failure if unmatched)
- `//~| NOTE sub` — continuation (same target line as previous)
- `//~^^ ERROR msg` — offset by N lines up

Test file header directives:
- `// test-mode: compile-fail` — explicit mode (overrides directory name)
- `// revisions: a b` — run test in multiple configurations
- `// compile-flags: --opt 2` — pass flags to compiler
- `// error-pattern: msg` — require this message somewhere in output
- `// needs-llvm` — skip if LLVM not available
- `// ignore` — skip this test
- `// timeout: 30` — set per-test timeout in seconds

### TestRunner API

```rust
use glyim_test::harness::{TestRunner, TestMode};
use std::time::Duration;

TestRunner::new("tests/compile-fail")
    .mode(TestMode::CompileFail)     // override mode
    .parallel(true)                   // run tests in parallel (default)
    .filter("type_mismatch")          // only run tests matching substring
    .timeout(Duration::from_secs(30)) // per-test timeout
    .max_concurrent(4)                // parallel thread count
    .frontend_only()                  // use FrontendOnlyCompiler instead of full pipeline
    .build()                          // discover tests, returns Result<TestPlan, TestDiscoveryError>
    .expect("discovery failed")
    .run();                           // execute; panics on failure

// Or use .execute() to get results without panicking:
let result = plan.execute();
println!("{} passed, {} failed", result.summary.passed, result.summary.failed);
```

### Environment Variables

- `GLYIM_BLESS=1` — update `.expected` files for ui tests
- `GLYIM_TEST_SHOW_OUTPUT=1` — verbose output on failure
- `GLYIM_TEST_JSON=1` — write JSON results to `target/test-results.json`
- `GLYIM_LLVM` — set to enable tests marked `needs-llvm`

### Directory Convention

```
tests/
├── compile-pass/    ← files that must compile without errors
│   └── basic_fn.g
├── compile-fail/    ← files that must produce expected errors
│   └── type_mismatch.g
└── ui/              ← files compared against .expected snapshots
    └── parse_fn.g
    └── parse_fn.expected
```

---

## TDD Workflow for Agents

1. **Create test module structure** in `src/tests/`
2. **Write ALL test cases** from your stream brief BEFORE implementing
3. **Verify tests compile** with `cargo check -p <crate>` (they will fail at runtime)
4. **Implement** until all tests pass
5. **Run full verification:**
   ```bash
   cargo test -p <crate>
   cargo clippy -p <crate> -- -D warnings
   cargo fmt --check -p <crate>
   cargo check --workspace
   ```

## Anti-Patterns to Avoid

| Don't | Do |
|-------|----|
| `Ty::from_raw(0)` | `Ty::ERROR` sentinel |
| `Substitution::from_raw(...)` | `ctx.intern_substitution(...)` |
| Integration tests in `tests/` | Unit tests in `src/tests/` |
| `todo!()` in non-test code | `tracing::warn!("STUB: reason")` |
| Silent no-ops / `let _ = x` | Visible stub with tracing warning |
| Stringly-typed errors | `GlyimDiagnostic` constructors |
| Writing tests after implementation | Write ALL tests first (TDD) |
| `InferenceTable::new_ty_var()` without `&mut TyCtxMut` | Always pass `&mut TyCtxMut` |
