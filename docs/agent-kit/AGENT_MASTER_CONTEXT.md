# Glyim Compiler — Master Agent Context

## Project Overview
Glyim is a from-scratch compiler for a Rust-like language, written in Rust. The codebase uses 25 crates organized in layers. You are implementing one stream of work within this project.

## Architecture Rules (NON-NEGOTIABLE)
1. **No `pub` signature changes.** If a public type or function exists in a crate you don't own, you MAY NOT modify it. If you need a change, add a `pub(crate)` helper instead.
2. **No new `pub` items in existing modules** without explicit approval. You MAY add new private modules and `pub(crate)` items freely.
3. **No `unsafe` in compiler crates.** Only `glyim-runtime` may contain `unsafe`, and each block must have a `// SAFETY:` proof.
4. **No `todo!()` in non-test code.** Use `tracing::warn!("STUB: {reason}")` for optional paths that need attention but won't crash.
5. **All stubs must be visible.** Silent no-ops (empty match arms, `let _ = x`) are forbidden in implementation code. Every stub must emit a warning on first execution.
6. **Tracing convention:** `trace` for hot paths, `debug` for inference, `info` for phases. Always `skip(self, ctx)`.
7. **Test-first:** Write all test cases from your stream's TDD plan BEFORE implementing. Tests must compile before implementation begins.

## Crate Dependency Rules
- Frontend crates (`glyim-syntax`, `glyim-frontend`, `glyim-hir`, `glyim-def-map`) NEVER depend on `glyim-type`.
- `glyim-lsp` depends ONLY on `glyim-db` (no LLVM transitive dependency).
- `glyim-db` does NOT use Salsa (removed for v0.1.0 honesty).
- `TyCtxMut` is `!Send + !Sync`. Post-typeck, only `TyCtx` (frozen, `Send + Sync`) is used.

## Key Type Contracts
- `Ty` does NOT implement `IdxLike`. Construction via `Ty::from_raw()` is `pub(crate)`. Use sentinels: `Ty::ERROR`, `Ty::NEVER`, `Ty::UNIT`, `Ty::BOOL`.
- `TypeLookup` trait bridges `TyCtx` and `TyCtxMut` for display and flag computation.
- `InferVar` has separate index types: `TyVar`, `IntVar`, `FloatVar`. Cross-kind construction is impossible by type.
- `compute_flags` is generic over `TypeLookup`: `fn compute_flags<L: TypeLookup>(kind: &TyKind, ctx: &L, depth: u32) -> TypeFlags`.
- `Substitution` is interned as `(u32 index, u16 len)`. Access via `ctx.substitution_args(sub)`.

## Error Handling
- Use `GlyimDiagnostic` for all errors. Constructors: `lex_error`, `parse_error`, `type_error`, `borrow_error`, `internal_error`.
- `DiagSink` collects diagnostics with error limiting (default 50).
- `CompResult<T> = Result<T, Vec<GlyimDiagnostic>>`.

## Git Convention
- Branch: `stream-S{XX}/v0.1.0` (e.g., `stream-S01/v0.1.0`)
- Commit format: `stream-S{XX}: description`
- Do NOT commit to `main` directly. Create a PR.
