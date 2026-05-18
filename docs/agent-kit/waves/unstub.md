# Stream Dispatcher Operational Playbook

This is your step-by-step command sequence for orchestrating the AI agent waves. Follow this exactly to guarantee zero merge conflicts and a compiling codebase at every step.

## Core Rules for the Dispatcher

1. **Never overlap waves:** Wave `N` agents must not be dispatched until Wave `N-1` is fully merged and `cargo check --workspace` passes.
2. **One agent = One branch:** Each agent works on an isolated branch `wave/{N}/{STREAM_ID}`.
3. **Merge order matters:** Within a wave, merge agents in the order listed. If an agent touches a crate that modifies a trait or macro used by later agents in the same wave, merge it first.
4. **Gates are absolute:** Do not merge a branch if any gate fails.

---

## Pre-Flight (Do this now)

```bash
# Ensure you are on main and clean
git checkout main
git pull origin main

# Verify the workspace currently compiles (or identify the baseline)
cargo check --workspace 2>&1 | head -n 20
```

---

## Phase 1: Foundation & The Enabler

**Start immediately.** This wave unlocks the rest of the compiler.

### Dispatch:
| Stream ID | Agent Branch | Agent Instructions |
|-----------|--------------|--------------------|
| **S01** | `wave/0/s01` | Modify `stub!` macro in `glyim-diag` from `compile_error!` to `unimplemented!()`. Add `stub_impl!`. Implement DiagSink and GlyimDiagnostic methods. |
| **S02** | `wave/0/s02` | Unstub `glyim-core`: Interner, Idx, Path, TargetInfo. |
| **S03** | `wave/0/s03` | Unstub `glyim-lang-core`, `glyim-lang-std`, `glyim-lang-alloc`. |

**Wait for:** S01, S02, S03 to finish.

### Merge Sequence:
```bash
# 1. Merge S01 FIRST (it changes the stub! macro, allowing the workspace to compile)
git checkout main && git merge --no-ff wave/0/s01 -m "Merge S01: Soft-stub migration"
cargo check --workspace  # MUST PASS

# 2. Merge S02
git checkout main && git merge --no-ff wave/0/s02 -m "Merge S02: Core unstubbing"
cargo check --workspace  # MUST PASS

# 3. Merge S03
git checkout main && git merge --no-ff wave/0/s03 -m "Merge S03: Lang libraries"
cargo check --workspace  # MUST PASS
```

---

## Phase 2: Syntax & Span

**Start:** After Phase 1 is merged to `main`.

### Dispatch:
| Stream ID | Agent Branch | Agent Instructions |
|-----------|--------------|--------------------|
| **S04** | `wave/1/s04` | Unstub `glyim-span`: HygieneCtx, Span, ExpnId. |
| **S05** | `wave/1/s05` | Unstub `glyim-syntax`: SyntaxKind, AstNode, GlyimLang. |
| **S06** | `wave/1/s06` | Unstub `glyim-runtime`: alloc, dealloc, panic FFI. |

**Wait for:** S04, S05, S06 to finish.

### Merge Sequence:
```bash
git checkout main && git merge --no-ff wave/1/s04 -m "Merge S04: Span"
git checkout main && git merge --no-ff wave/1/s05 -m "Merge S05: Syntax"
git checkout main && git merge --no-ff wave/1/s06 -m "Merge S06: Runtime"
cargo check --workspace  # MUST PASS
```

---

## Phase 3: VFS & Diagnostics Implementation

**Start:** After Phase 2 is merged.

### Dispatch:
| Stream ID | Agent Branch | Agent Instructions |
|-----------|--------------|--------------------|
| **S07** | `wave/2/s07` | Unstub `glyim-vfs` and complete `glyim-diag` implementations. |

**Wait for:** S07 to finish.

### Merge Sequence:
```bash
git checkout main && git merge --no-ff wave/2/s07 -m "Merge S07: VFS & Diag completion"
cargo check --workspace  # MUST PASS
```

---

## Phase 4: The Big One (Frontend, HIR, DefMap, Meta, Type)

**Start:** After Phase 3 is merged. **This is the critical path. Monitor closely.**

### Dispatch:
| Stream ID | Agent Branch | Agent Instructions | Special Note |
|-----------|--------------|--------------------|--------------|
| **S08** | `wave/3/s08` | Unstub `glyim-frontend`: Lexer & Parser. | |
| **S09** | `wave/3/s09` | Unstub `glyim-hir`: Exhaustive matches, foreign fns. **Apply CR-002** (change `lower_crate_for_pipeline` signature). | Merge S09 before S11/S20 rely on it. |
| **S10** | `wave/3/s10` | Unstub `glyim-def-map`: Name resolution. **Apply CR-007** (add `Interner` param). | Merge S10 before S16 relies on it. |
| **S11** | `wave/3/s11` | Unstub `glyim-meta`: Macro expansion. | |
| **S12** | `wave/3/s12` | Unstub `glyim-type`: TyCtxMut, TypeLookup. **Apply CR-001, CR-006, CR-008** (Add `fn_sig`, `body_ty`, `field_ty` to traits). | **CRITICAL:** Merge S12 first. S13, S14, S15 depend on the new trait methods. |

**Wait for:** S08, S09, S10, S11, S12 to finish.

### Merge Sequence:
```bash
# 1. MERGE S12 FIRST - It extends TypeLookup which others depend on compiling
git checkout main && git merge --no-ff wave/3/s12 -m "Merge S12: Type system & CRs"
cargo check --workspace  # MUST PASS

# 2. Merge S10 - Extends build_def_map signature
git checkout main && git merge --no-ff wave/3/s10 -m "Merge S10: Def-Map & CR-007"
cargo check --workspace

# 3. Merge S09 - Extends lower_crate_for_pipeline signature
git checkout main && git merge --no-ff wave/3/s09 -m "Merge S09: HIR & CR-002"
cargo check --workspace

# 4. Merge S08 and S11 in any order
git checkout main && git merge --no-ff wave/3/s08 -m "Merge S08: Frontend"
git checkout main && git merge --no-ff wave/3/s11 -m "Merge S11: Meta"
cargo check --workspace  # MUST PASS
```

---

## Phase 5: MIR, Solver, Layout

**Start:** After Phase 4 is merged.

### Dispatch:
| Stream ID | Agent Branch | Agent Instructions | Special Note |
|-----------|--------------|--------------------|--------------|
| **S13** | `wave/4/s13` | Unstub `glyim-mir`: `Place::ty` (uses CR-008 `field_ty` added in S12). | |
| **S14** | `wave/4/s14` | Unstub `glyim-solve`: Cycle detection, Coerce, **Apply CR-004** (InferenceSnapshot). | Merge S14 before S16 relies on snapshots. |
| **S15** | `wave/4/s15` | Unstub `glyim-layout`: LayoutComputer, niche encoding. | |

**Wait for:** S13, S14, S15 to finish.

### Merge Sequence:
```bash
# Merge S14 first (adds InferenceSnapshot)
git checkout main && git merge --no-ff wave/4/s14 -m "Merge S14: Solver & CR-004"
# Merge S13, S15
git checkout main && git merge --no-ff wave/4/s13 -m "Merge S13: MIR"
git checkout main && git merge --no-ff wave/4/s15 -m "Merge S15: Layout"
cargo check --workspace  # MUST PASS
```

---

## Phase 6: Typeck, Borrowck, Opt, Interp

**Start:** After Phase 5 is merged.

### Dispatch:
| Stream ID | Agent Branch | Agent Instructions | Special Note |
|-----------|--------------|--------------------|--------------|
| **S16** | `wave/5/s16` | Unstub `glyim-typeck`: MethodCalls, instantiation. **Apply CR-005** (TypeckResult accessors). | Merge before S20. |
| **S17** | `wave/5/s17` | Unstub `glyim-borrowck`: NLL, liveness, two-phase. | |
| **S18** | `wave/5/s18` | Unstub `glyim-opt`: Constant prop, DCE. | |
| **S19** | `wave/5/s19` | Unstub `glyim-mir-interp`: Discriminant, projections, binary ops. | |

**Wait for:** S16, S17, S18, S19 to finish.

### Merge Sequence:
```bash
# Merge S16 first (adds TypeckResult accessors)
git checkout main && git merge --no-ff wave/5/s16 -m "Merge S16: Typeck & CR-005"
# Merge the rest
git checkout main && git merge --no-ff wave/5/s17 -m "Merge S17: Borrowck"
git checkout main && git merge --no-ff wave/5/s18 -m "Merge S18: Opt"
git checkout main && git merge --no-ff wave/5/s19 -m "Merge S19: Interp"
cargo check --workspace  # MUST PASS
```

---

## Phase 7: Lower & Codegen

**Start:** After Phase 6 is merged.

### Dispatch:
| Stream ID | Agent Branch | Agent Instructions | Special Note |
|-----------|--------------|--------------------|--------------|
| **S20** | `wave/6/s20` | Unstub `glyim-lower`: Pattern destructuring, field resolution. **Apply CR-003, CR-010** (Extend LowerCtx). | Merge S20 before S24 relies on it. |
| **S21** | `wave/6/s21` | Unstub `glyim-codegen`: Bytecode projections, Assert, Drop. | |

**Wait for:** S20, S21 to finish.

### Merge Sequence:
```bash
# Merge S20 first (extends LowerCtx trait)
git checkout main && git merge --no-ff wave/6/s20 -m "Merge S20: Lower & CRs"
git checkout main && git merge --no-ff wave/6/s21 -m "Merge S21: Codegen"
cargo check --workspace  # MUST PASS
```

---

## Phase 8: LLVM & Database

**Start:** After Phase 7 is merged.

### Dispatch:
| Stream ID | Agent Branch | Agent Instructions |
|-----------|--------------|--------------------|
| **S22** | `wave/7/s22` | Unstub `glyim-codegen-llvm`: Types, aggregates, PassModes, Asserts. |
| **S23** | `wave/7/s23` | Unstub `glyim-db`: Database RwLock internals. |

**Wait for:** S22, S23 to finish.

### Merge Sequence:
```bash
git checkout main && git merge --no-ff wave/7/s22 -m "Merge S22: LLVM Backend"
git checkout main && git merge --no-ff wave/7/s23 -m "Merge S23: Database"
cargo check --workspace  # MUST PASS
```

---

## Phase 9: Pipeline Assembly

**Start:** After Phase 8 is merged.

### Dispatch:
| Stream ID | Agent Branch | Agent Instructions |
|-----------|--------------|--------------------|
| **S24** | `wave/8/s24` | Unstub `glyim-pipeline`: Generic instantiation, drop glue (DropGlueProvider), ADT resolution. |

**Wait for:** S24 to finish.

### Merge Sequence:
```bash
git checkout main && git merge --no-ff wave/8/s24 -m "Merge S24: Pipeline"
# Run a full integration test here if possible
cargo test --workspace  # MUST PASS
```

---

## Phase 10: User Interface (CLI & LSP)

**Start:** After Phase 9 is merged.

### Dispatch:
| Stream ID | Agent Branch | Agent Instructions |
|-----------|--------------|--------------------|
| **S25** | `wave/9/s25` | Unstub `glyim-cli`: Clap args, run pipeline. |
| **S26** | `wave/9/s26` | Unstub `glyim-lsp`: Router, find_references, build_from_hir. |

**Wait for:** S25, S26 to finish.

### Merge Sequence:
```bash
git checkout main && git merge --no-ff wave/9/s25 -m "Merge S25: CLI"
git checkout main && git merge --no-ff wave/9/s26 -m "Merge S26: LSP"
cargo check --workspace  # MUST PASS
```

---

## Phase 11: Test Infra & Build Tool

**Start:** After Phase 10 is merged.

### Dispatch:
| Stream ID | Agent Branch | Agent Instructions |
|-----------|--------------|--------------------|
| **S27** | `wave/10/s27` | Unstub `glyim-test`: Update mocks (LowerCtx, InferenceSnapshot, etc.). |
| **S28** | `wave/10/s28` | Unstub `glyip`: cmd_new, cmd_build, Cache, Fingerprints. |

**Wait for:** S27, S28 to finish.

### Merge Sequence:
```bash
git checkout main && git merge --no-ff wave/10/s27 -m "Merge S27: Test Infra"
git checkout main && git merge --no-ff wave/10/s28 -m "Merge S28: Glyip"
cargo test --workspace  # MUST PASS - All stubs should be gone from main crates
```

---

## Phase 12: Dev Tools

**Start:** After Phase 11 is merged.

### Dispatch:
| Stream ID | Agent Branch | Agent Instructions |
|-----------|--------------|--------------------|
| **S29** | `wave/11/s29` | Unstub `glyim-pilot`: Server, orchestrator, gates. |

**Wait for:** S29 to finish.

### Merge Sequence:
```bash
git checkout main && git merge --no-ff wave/11/s29 -m "Merge S29: Pilot Dev Tools"
cargo check --workspace
```

---

## Post-Merge Final Validation

Once all waves are merged, run the final strict stub elimination checks:

```bash
# Ensure no STUB: warnings remain
! grep -rn "STUB:" crates/ tools/ src/

# Ensure no unimplemented!() in locked public items
! grep -rn "unimplemented!" crates/*/src/lib.rs

# Full test suite
cargo test --workspace --all-features

# Clippy check
cargo clippy --workspace -- -D warnings
```

If all pass, the unstubbing is complete.
