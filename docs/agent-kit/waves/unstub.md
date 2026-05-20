# Human Branch Merger Plan – What to Merge and When

This plan assumes each stream produces a **separate pull request (PR)** from its own branch, and you (the human) will merge them in the specified order.

## Merge Rules

- **Within a wave**: all streams are independent (no file conflicts). Merge in **any order**.
- **Between waves**: you must merge **all streams** of a wave before moving to the next wave (due to compiler pipeline dependencies).
- **After each wave**: run the full test suite (`cargo test --workspace`) to catch integration issues early.

---

## Wave 0 – Frontend & Lexer (2 streams)

| Stream | Files changed | Dependencies | Merge after |
|--------|---------------|--------------|-------------|
| S01 | `glyim-frontend/src/lexer.rs` | none | any order |
| S02 | `glyim-frontend/src/parser/*.rs` | none | any order |

**Merge all PRs from S01, S02** (order doesn’t matter).  
**Then run tests** – the parser now correctly handles number suffixes, macro repetitions, pub(crate), etc.

---

## Wave 1 – HIR & Type System (2 streams)

| Stream | Files changed | Dependencies | Merge after |
|--------|---------------|--------------|-------------|
| S03 | `glyim-hir/src/lower/*.rs` | S02 (parser) | Wave 0 complete |
| S04 | `glyim-type/src/{auto_trait,ty_ctx,ty_ctx_mut}.rs` | none | Wave 0 complete |

**Merge S03 and S04 in any order** (they touch different crates).  
**Run tests** – HIR now covers all expression/pattern types; auto traits and field types work.

---

## Wave 2 – Type Checker (1 stream)

| Stream | Files changed | Dependencies | Merge after |
|--------|---------------|--------------|-------------|
| S05 | `glyim-typeck/src/{check_expr,check_pat,tyconv}.rs` | S03, S04 | Wave 1 complete |

**Merge S05** (only one stream).  
**Run tests** – method resolution, for‑loops, pattern checking now work.

---

## Wave 3 – MIR Lowering & Interpreter (2 streams)

| Stream | Files changed | Dependencies | Merge after |
|--------|---------------|--------------|-------------|
| S06 | `glyim-lower/src/{lower_rvalue,lower,lower_terminator}.rs` | S05 | Wave 2 complete |
| S07 | `glyim-mir-interp/src/lib.rs` | S06 | Wave 2 complete (needs MIR) |

**Merge S06 first** (produces MIR), **then S07** (interpreter depends on MIR).  
**Run tests** – for‑loops, closures, pattern matching and interpreter now fully functional.

---

## Wave 4 – Code Generation (2 streams)

| Stream | Files changed | Dependencies | Merge after |
|--------|---------------|--------------|-------------|
| S08 | `glyim-codegen/src/lib.rs` | S06, S07 | Wave 3 complete |
| S09 | `glyim-codegen-llvm/src/{types,lower}.rs` | S06, S07, S08 | Wave 3 complete (S08 not strictly required but recommended) |

**Merge S08 then S09** (bytecode backend first, LLVM backend may reuse some logic).  
**Run tests** – both backends now handle all MIR constructs.

---

## Wave 5 – Pipeline, LSP, Build Tool (3 streams)

| Stream | Files changed | Dependencies | Merge after |
|--------|---------------|--------------|-------------|
| S10 | `glyim-pipeline/src/{mono_cache,pipeline_context,mono}.rs` | S06, S07, S08, S09 | Wave 4 complete |
| S11 | `glyim-lsp/src/{reference_graph,driver,navigation}.rs` | S03, S05 | Wave 4 complete |
| S12 | `glyip/src/{dep,fingerprint,cache}.rs` | none (independent) | Wave 4 complete |

**Merge S10, S11, S12 in any order** (they touch different crates).  
**Run tests** – monomorphization, drop glue, LSP references, and build tool all work.

---

## Wave 6 – AI Pilot (1 stream)

| Stream | Files changed | Dependencies | Merge after |
|--------|---------------|--------------|-------------|
| S13 | `glyim-pilot/src/{gates,orchestrator,dispatch,commit}/*.rs` | S10, S11, S12 | Wave 5 complete |

**Merge S13** (last stream).  
**Run full test suite** – all stubs removed, all gates and orchestrator functional.

---

## Final Verification

After merging **all waves**, run:

```bash
cargo test --workspace
cargo clippy -- -D warnings
cargo fmt -- --check
grep -r "STUB:" crates/ --include="*.rs"   # should return nothing
grep -r "unimplemented!" crates/ --include="*.rs" | grep -v "macro_rules"   # should return nothing
```

If all clear, the unstubbing is **complete**.

---

## Quick Reference Table

| Wave | Streams to merge | Merge order | Test after |
|------|----------------|-------------|-------------|
| 0 | S01, S02 | any | ✅ |
| 1 | S03, S04 | any | ✅ |
| 2 | S05 | only one | ✅ |
| 3 | S06 then S07 | S06 → S07 | ✅ |
| 4 | S08 then S09 | S08 → S09 | ✅ |
| 5 | S10, S11, S12 | any | ✅ |
| 6 | S13 | only one | ✅ |

**Remember**: after each wave, run `cargo test --workspace` before merging the next wave’s PRs. This catches integration issues early and keeps the main branch stable.
