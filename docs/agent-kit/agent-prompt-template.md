You are implementing Stream S{ID}: {NAME} for the Glyim compiler.

## Instructions

1. Read your stream brief completely before writing any code.
2. You MUST follow the Test-Driven Development (TDD) workflow:
   a. Create the `src/tests/` module structure in your owned crate.
   b. Write ALL test cases from your brief in `src/tests/` — NOT in the `tests/` directory.
   c. Verify your tests compile with `cargo check -p <crate>`. They may fail at runtime until implementation is done.
3. Implement features until all tests pass.
4. Run ALL verification commands from your brief:
   ```bash
   cargo test -p <crate>
   cargo clippy -p <crate> -- -D warnings
   cargo fmt --check -p <crate>
   cargo check --workspace
   ```
5. Output: Provide complete modified files using the bash script format. Never truncate, never use placeholders, never omit lines.

## Critical Rules
- NEVER modify `pub` interfaces in crates you don't own.
- NEVER use `todo!()` in non-test code — use `tracing::warn!("STUB: reason")`.
- NEVER write integration tests in `tests/` — write unit tests in `src/tests/`.
- ALWAYS use `glyim-test` dev-dependency for test helpers, mocks, and assertions.
- ALWAYS use sentinels (`Ty::ERROR`, `Ty::NEVER`, etc.) instead of `Ty::from_raw()`.
- ALWAYS use `ctx.intern_substitution(vec![...])` instead of `Substitution::from_raw()`.
- ALWAYS pass `&mut TyCtxMut` to `InferenceTable::new_ty_var/int_var/float_var`.
- ALWAYS emit one fenced bash code block per message with the script inside.
- ALWAYS use `echo` for logging — no `#` comment lines.
- ALWAYS write complete file content — never truncate, never use `...` placeholders.
