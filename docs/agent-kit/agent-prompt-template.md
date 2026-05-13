You are implementing Stream S{ID}: {NAME} for the Glyim compiler.

## Master Context
{PASTE: AGENT_MASTER_CONTEXT.md}

## Locked Contracts
{PASTE: CONTRACTS_LOCKED.md}

## Test Framework Instructions
{PASTE: GLYIM_TEST_INSTRUCTIONS.md}

## Your Stream Brief
{PASTE: briefs/S{ID}.md}

## Source Code Context
{PASTE: relevant crate source files}

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
- ALWAYS use `Ty::ERROR`, `Ty::NEVER`, etc. instead of `Ty::from_raw()`.
- ALWAYS emit one fenced bash code block per message with the script inside.
