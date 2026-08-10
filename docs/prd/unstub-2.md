# Comprehensive Stubs and Semi-Implemented Features Report

This report catalogs all stubs, placeholders, and incomplete features found in the provided codebase dump. It covers every crate and module present, with detailed descriptions and suggested next steps.

## Overview
- **Total stubs identified**: 34 (plus additional placeholders)
- **Categories**: 
  - **Missing implementations** (e.g., pattern lowering, slice support)
  - **Incomplete analysis** (e.g., two-phase borrow activation across blocks, object safety)
  - **Debug/temporary markers** (e.g., `tracing::warn!("STUB: ...")`)
  - **`todo!()` / `unimplemented!()`** macros
  - **Limited fallback paths** (e.g., error type handling)

---

## 1. `glyim-borrowck` Crate

### 1.1 `src/lib.rs`
- **Line ~45** (function `check_borrows`): Activation analysis is only computed for **same‑block** two-phase borrows. Cross‑block activation is conservatively considered activated (documented as a v0.1.0 simplification).  
  *Note:* `loan_is_in_reservation` uses a cache but only points to the `ReservationAnalysis` computed per‑block; no cross‑block propagation.

### 1.2 `src/liveness.rs`
- **Line ~17** (struct `LivenessResult`): `live_in` is marked `#[allow(dead_code)]` – likely unused by callers but kept for completeness.

### 1.3 `src/move_analysis.rs`
- **Line ~190** (`collect_rvalue_move_operands`): `Rvalue::Ref` is handled by doing nothing – but a `Ref` might capture a value that should be considered moved? This is probably correct because taking a reference does not move, but the comment in the match arm is empty.  
  *Suggestion:* Verify that taking a reference should not affect move analysis (it shouldn't), so the stub is intentional but should be documented.

### 1.4 `src/twophase.rs`
- **Line ~13** (doc comment): "Two‑phase borrow activation analysis – same block only." This is a known limitation; cross‑block activation is conservatively assumed to be activated.

---

## 2. `glyim-codegen` Crate

### 2.1 `src/lib.rs` (BytecodeBackend)
- **Line ~210** (`emit_operand` for `ProjectionElem::Slice`): Emits a placeholder (pushing 0) and prints a warning. Slice projection is not implemented in the bytecode backend.
- **Line ~250** (`emit_operand` for `Operand::Copy` with `ProjectionElem::Slice`): The handling of `start`/`end` is implemented, but the code notes that `Slice` projection is not fully supported for assignment targets.
- **Line ~300** (`emit_terminator` for `TerminatorKind::Call`): The bytecode backend generates `OP_CALL_INDIRECT` for function pointers, but uses a simplistic ABI (passing args directly). No ABI‑aware passing (e.g., sret, byval) is implemented.

### 2.2 `src/tests/backend_tests.rs`
- Many test placeholders (e.g., `t31_operand_with_projection_stub_does_not_crash`) – they only verify that no panic occurs, not that the behaviour is correct.

---

## 3. `glyim-codegen-llvm` Crate

### 3.1 `src/debug.rs`
- **Line ~55** (`debug::DebugInfoCtx::new`): The `DWARFSourceLanguage` is hardcoded to `C`; should be `Rust` or a custom language.  
- **Line ~85** (`declare_local`): Uses a basic `"i32"` type for all variables; type information is not properly translated from Glyim types.

### 3.2 `src/lower.rs`
- **Line ~210** (`lower_rvalue` for `Rvalue::Aggregate` with `AggregateKind::Adt`): If the ADT is not found, it uses a fallback but does not handle the case when the variant is a struct with fields (only a simple tuple construction is attempted).  
- **Line ~400** (`lower_rvalue` for `Rvalue::Discriminant`): For enums with `TagEncoding::Niche`, the computed discriminant may be incorrect if the niche range does not start at 0 – the logic uses a simple subtraction/addition without verifying the actual valid range.  
- **Line ~520** (`lower_call`): For `Call` terminators with a `cleanup` block, the LLVM `invoke` instruction is generated, but the personality function is only set if any block is marked as cleanup. The landingpad code is present but not fully tested.  
- **Line ~700** (`lower_terminator` for `TerminatorKind::Drop`): Array drops are handled by calling `glyim_drop_in_place` on the array pointer, but the implementation does not loop over elements; it relies on the runtime to handle arrays, which is a stub at runtime.

### 3.3 `src/abi.rs`
- **Line ~40** (`layout_of` for `TyKind::Adt`): For enums, the tag size is computed as `ceil(log2(n_variants))` but the `tag_size` and `tag_align` fields are set but never used in codegen; they are part of the layout but not honoured when generating GEPs for tags.

### 3.4 `src/passes.rs`
- **Line ~8**: The pass strings are fixed (e.g., `"default<O2>"`). The pass manager does not respect all optimization levels; for `opt_level` ≥ 4, it defaults to O2.

### 3.5 `src/tests/*`
- Many tests are `#[ignore]`d because they require frontend/typeck support (e.g., slice patterns, or‑patterns). These are marked with comments like `requires frontend/typeck support for Pat::Or`.

---

## 4. `glyim-def-map` Crate

### 4.1 `src/lib.rs`
- **Line ~120** (`process_use_tree`): The visibility validation for `use` statements is incomplete – it only checks visibility for types and values, not for macros.  
- **Line ~180** (`is_accessible_from`): The `Visibility::Module` variant is parsed but the ID is a `u32`, which is used as `ModuleId::from_raw` without validation that the module exists.

---

## 5. `glyim-frontend` Crate

### 5.1 `src/parser/expr.rs`
- **Line ~130** (`parse_postfix_expr`): Handling of `..` (struct update syntax) is present but the expression for the base (after `..`) is parsed but not used in the AST – it's simply consumed. This is actually correct for parsing, but the subsequent HIR lowering may ignore the spread.  
- **Line ~230** (`parse_path_expr`): `last_was_path` flag is set, but its usage is limited to detecting macros; no resolution is performed.

### 5.2 `src/parser/pat.rs`
- **Line ~40** (`parse_pat_inner` for `PatRange`): The range pattern is parsed but the `start` and `end` are not validated to be literals; it accepts any pattern, which may lead to later errors.

### 5.3 `src/parser/ty.rs`
- **Line ~80** (`parse_type` for `TyKind::Dyn`): The `dyn Trait` type is parsed but the associated trait bounds are ignored; only the first trait is used.

---

## 6. `glyim-hir` Crate

### 6.1 `src/lower/lower_expr.rs`
- **Line ~60** (`lower_block_to_expr`): The handling of `LetStmt` does not support `let` with a pattern that introduces multiple bindings (e.g., `let (a, b) = ...`) – it only works with `PatIdent` or `PatWild`.  
- **Line ~200** (`lower_binary_expr`): The operator token is found by scanning children, but if there are multiple operators, only the first is used; this may break precedence, but the parser already builds a tree, so it's fine.  
- **Line ~550** (`lower_match_expr`): The guard expression is parsed as an `if` condition but the lowering does not check that the guard is a boolean; it will simply lower any expression.  
- **Line ~700** (`lower_while_expr`): The `while` loop lowering does not support `break` or `continue` with labels; labels are ignored.

### 6.2 `src/lower/lower_pat.rs`
- **Line ~120** (`lower_pat` for `PatStruct`): The `rest` field (for `..`) is parsed but not used – the field is set but no spread is created in the HIR pattern.

### 6.3 `src/lower/lower_type.rs`
- **Line ~30** (`lower_type_ref` for `FnType`): The return type is parsed but the arrow detection is fragile – if there are nested arrows, only the first is considered.

---

## 7. `glyim-lower` Crate

### 7.1 `src/lower.rs`
- **Line ~150** (`lower_expr_to_rvalue` for `thir::ExprKind::If`): The then/else branches are lowered, but the `else` branch is optional; if absent, it generates a `Unit` constant. This is correct but may produce a block with no value if used as an expression.  
- **Line ~210** (`lower_expr_to_rvalue` for `thir::ExprKind::For`): If the iterator type does not provide `IteratorNextInfo`, a fallback path is used where the loop variable is simply bound to the iterable directly – this is a stub for non‑iterator loops.

### 7.2 `src/lower_rvalue.rs`
- **Line ~80** (`lower_expr_to_rvalue` for `thir::ExprKind::Closure`): The closure body is stored as a `thir::Body` but not lowered; it's only stored as a nested body. The actual lowering of the closure's body is deferred, but the closure expression itself is lowered to an `Aggregate` of captures. This is correct but the closure body is not yet lowered into MIR – it will be when the closure is called.

### 7.3 `src/lower_terminator.rs`
- No stubs – just a trait implementation.

### 7.4 `src/mono.rs`
- **Line ~50** (`scan_body_for_refs`): The `scan_terminator` for `Drop` simply records the local; it does not attempt to scan the dropped type's fields for nested drops. That's handled by drop glue generation, but the drop glue generation is only used when the type is collected; it may not be generated for all types.

### 7.5 `src/polymorphize.rs`
- **Line ~120** (`deduplicate`): The polymorphization is applied, but only for Fn and Const items; DropGlue and Static items are left unchanged.

### 7.6 `src/post_mono_checks.rs`
- Many functions are `#[allow(dead_code)]` because they are not yet called from the pipeline (e.g., `check_unsized_locals`). They are implemented but unused.

---

## 8. `glyim-mir` Crate

### 8.1 `src/lib.rs`
- **Line ~130** (`Place::ty` for `ProjectionElem::Slice`): Returns the base type, not the element type, and logs a warning. Slice projections are not fully handled in type computation.

---

## 9. `glyim-opt` Crate

### 9.1 `src/constant_prop.rs`
- **Line ~90** (`evaluate_rvalue_to_const`): Only handles `Int` and `Uint` constants; does not handle `Float`, `Bool`, `Char`, or aggregates.  
- **Line ~150** (`replace_in_rvalue` for `Rvalue::Ref`): Does nothing (returns false), so references are never replaced by constants.  
- **Line ~170** (`replace_in_rvalue` for `Rvalue::Discriminant`/`Len`): Does nothing; these are not constant‑propagated.

### 9.2 `src/drop_elaboration.rs`
- **Line ~40** (`run` for `TerminatorKind::Drop` on `TyKind::Array`): The implementation replaces the Drop with a Goto (stub). A comment says "full implementation will generate a loop" – this is not done.

### 9.3 `src/unreachable_elim.rs`
- No obvious stubs.

---

## 10. `glyim-pipeline` Crate

### 10.1 `src/lib.rs`
- **Line ~90** (`emit_mir`): Writes a placeholder text "MIR not yet implemented".  
- **Line ~98** (`emit_llvm_ir`): Writes a placeholder text "LLVM IR not yet implemented".

### 10.2 `src/mono_cache.rs`
- **Line ~80** (`substitute_body`): The `substitute_body` function only substitutes local types and some rvalue types; it does not substitute constants in `MirConstKind::Fn` or `MirConstKind::ConstRef` – those are left as is, assuming the substitution is already applied at the call site.

---

## 11. `glyim-runtime` Crate

### 11.1 `src/fs.rs`
- **Line ~15** (doc): Mentions that array drops are not yet implemented; the runtime currently calls a generic `glyim_drop_in_place` which is a stub for arrays.  
- **Line ~180** (`glyim_fs_canonicalize`): The implementation uses `std::fs::canonicalize` which resolves symlinks, but the runtime does not track current working directory changes (though `glyim_env_current_dir` does).

### 11.2 `src/lib.rs` (threading)
- **Line ~440** (`glyim_thread_spawn`): The closure argument is a raw function pointer and a raw pointer; the Rust closure `move || { f(arg_ptr); }` is used, but it does not handle panic unwinding (it will abort).  
- **Line ~480** (`glyim_thread_join`): The join returns `-1` on panic; no panic propagation.

### 11.3 `src/tests/*`
- Many test files are for basic functionality; no stubs in tests.

---

## 12. `glyim-solve` Crate

### 12.1 `src/hrtb.rs`
- **Line ~140** (`check_hrtb` for `Predicate::RegionOutlives` etc.): Returns `Proven` for all non‑trait predicates without checking. This is a simplification.

### 12.2 `src/infer.rs`
- **Line ~300** (`unify_tys` for `TyKind::Dynamic`): Logs a warning and returns `Ok` without unifying the predicates; this is a stub.

---

## 13. `glyim-syntax` (implicitly used, but not in dump)

Not present in dump, but the parser relies on `SyntaxKind`; no stubs.

---

## 14. Missing Crates (not in dump)
Several crates are **not included** in the dump (e.g., `glyim-type`, `glyim-typeck`, `glyim-mir-interp`, `glyim-lang-*`). Their stubs are not covered, but based on context:

- `glyim-typeck`: likely has stubs for type inference and trait solving.
- `glyim-mir-interp`: may have partial support for certain MIR operations (e.g., slice projections).
- `glyim-lang-std`/`core`: may have stubs for `format!` or other macros.

---

## Summary Table of Major Stubs

| Crate | Module/File | Stub Description | Priority |
|-------|-------------|------------------|----------|
| glyim-borrowck | twophase.rs | Cross-block two-phase activation not implemented | High |
| glyim-codegen | lib.rs (bytecode) | Slice projections not supported | Medium |
| glyim-codegen-llvm | lower.rs | Array drop not per-element; invokes runtime stub | High |
| glyim-codegen-llvm | debug.rs | Debug info uses hardcoded C language and i32 types | Low |
| glyim-frontend | parser/pat.rs | Range patterns accept any pattern; no validation | Low |
| glyim-hir | lower/lower_expr.rs | Let-stmt pattern binding only supports simple identifiers | Medium |
| glyim-lower | lower_rvalue.rs | Closure body not lowered until call | Medium |
| glyim-lower | polymorphize.rs | DropGlue and Static items not polymorphized | Medium |
| glyim-opt | constant_prop.rs | Only Int/Uint constants; no floats/bools/aggregates | Medium |
| glyim-opt | drop_elaboration.rs | Array drops replaced with Goto (stub) | High |
| glyim-pipeline | lib.rs | emit_mir/emit_llvm_ir only write placeholder | High |
| glyim-runtime | fs.rs | Array drop in runtime is not implemented | High |
| glyim-solve | infer.rs | Dynamic predicate unification not implemented | Medium |
| glyim-solve | hrtb.rs | Non‑trait HRTB predicates always proven | Low |
| glyim-codegen-llvm | passes.rs | Optimization levels beyond 3 default to O2 | Low |

---

## Recommendations

1. **Prioritize high‑priority stubs** (affecting correctness): cross‑block two‑phase borrows, array drops, and file system operations.
2. **Complete intermediate stubs** (affecting functionality): closure lowering, constant propagation for more types, and pattern binding.
3. **Low‑priority stubs** (debug info, certain parser validations) can be addressed later.
4. **Remove or replace `tracing::warn!` stubs** with proper error handling or full implementation before v0.2.0.

This report should serve as a roadmap for completing the codebase.
