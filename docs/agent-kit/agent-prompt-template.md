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

## Output Skill: plan-to-cat-scripts (MANDATORY)

You MUST follow the plan-to-cat-scripts skill exactly. This is non-negotiable.

### Skill Summary
- Every message is exactly one fenced bash code block -- no other text, no explanations.
- First script: Set STREAM_ID, create branch stream-SXX/v0.1.0 from main, checkout.
- Subsequent scripts: Set STREAM_ID, assume branch already checked out.
- Fix scripts: Set STREAM_ID, assume branch already checked out.
- File writes: Use heredoc with unique delimiters that do not appear in content.
- Patches: Trivial single-line use sed. Everything else use Python with temp files.
- No hash-comment lines: Every action logged with echo.
- No truncation: Set INCOMPLETE=true and continue in next message.
- Compile check: Runs at end, failure blocks commit but never halts script.
- Commits: Prefixed with stream-SXX:.
- Error logs: User pastes terminal output then you emit one surgical fix script.

### Critical Rules
- NEVER modify pub interfaces in crates you do not own.
- NEVER use todo!() in non-test code -- use tracing::warn!("STUB: reason").
- NEVER write integration tests in tests/ -- write unit tests in src/tests/.
- ALWAYS use glyim-test dev-dependency for test helpers, mocks, and assertions.
- ALWAYS use sentinels (Ty::ERROR, Ty::NEVER, etc.) instead of Ty::from_raw().
- ALWAYS use ctx.intern_substitution(vec![...]) instead of Substitution::from_raw().
- ALWAYS pass andmut TyCtxMut to InferenceTable methods.
- ALWAYS emit one fenced bash code block per message -- no other text.
- ALWAYS use echo for logging -- no hash-comment lines.
- ALWAYS write complete file content -- never truncate, never use placeholders.
- ALWAYS create the stream branch in the first script.
- ALWAYS prefix commit messages with stream-SXX:.
