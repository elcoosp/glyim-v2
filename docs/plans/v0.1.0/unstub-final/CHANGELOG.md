# Stub Remediation — Work Changelog

Plan: `docs/plans/v0.1.0/unstub-final/`
Each tier is implemented and committed atomically. Tests are run with
`cargo nextest run -p <crate>` for affected crates.

Legend: ✅ done · 🚧 in progress · ⏳ pending

## Tier 0 — Soundness bugs (silently wrong output)
- ✅ 0.1 `get_element_size` now uses real `LayoutComputer` (mir-interp + glyim-layout dep). Verified: i32 element size == 4, [i32;4] == 16 bytes.
- ✅ 0.2 `ConstantIndex`/`Subslice` write arms implemented (no longer panic). Verified by tier0 tests.
- ✅ 0.3 `CastKind::PtrToPtr`/`FnPtrToPtr` confirmed correct no-op; added explanatory comment.
- ✅ 0.4 `Drop` terminator documented as no-op scope decision; added `with_panics_unwind` flag + debug assert; module doc notes limitation.
- ✅ 0.5 `Len` non-integer `ConstKind` already returns `Err` (pre-existing) — no change needed.

## Tier 1 — Missing core semantics
- ⏳ 1.1 closure capture analysis
- ⏳ 1.1b real closure ADT type
- ⏳ 1.2 vtable generation (ImplDef.items schema + VTableComputer + pipeline wiring)
- ⏳ 1.3 Iterator::next real resolution
- ⏳ 1.4 Range lowering bug
- ⏳ 1.5 const-eval expression coverage (Loop/While/Flow/Call/etc)
- ⏳ 1.6 drop elaboration per-projection (move-path tree)
- ⏳ 1.7 dynamic range slicing

## Tier 2 — Trait system
- ⏳ 2.1 coherence overlap ignores generics
- ⏳ 2.2 HRTB provable cases
- ⏳ 2.3 object safety associated types & supertraits

## Tier 3 — Build tool (glyip)
- ⏳ 3.1 transitive dependency resolution
- ⏳ 3.2 glyip cmd_test executes tests
- ⏳ 3.3 registry-disabled error message

## Tier 4 — Macro system
- ⏳ 4.1 fragment-spec matching (Stage A + B)
- ⏳ 4.2 line!/column! from SourceMap
- ⏳ 4.3 include! CWD-relative fix
- ⏳ 4.4 stringify! normalization

## Tier 5 — Codegen / debug-info
- ⏳ 5.1 over-alignment fallback + set_alignment
- ⏳ 5.2 DWARF pointer/slice debug types
- ⏳ 5.3 fn_sig fallback -> hard error
- ⏳ 5.4 bytecode backend Subslice/ConstantIndex scaling

## Tier 6 — LSP
- ⏳ 6.1-6.5 reference graph / rename / completion / unused-imports

## Tier 7 — Test harness
- ⏳ 7.1-7.4 real linking+execution + mock wiring
