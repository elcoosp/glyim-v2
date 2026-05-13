# Stream S{ID}: {NAME}

## Mission
Implement {SCOPE_SUMMARY} for the Glyim compiler.

## What You Own
{OWNED_CRATES_AND_MODULES}

## What You May NOT Modify
Any `pub` interface listed in `CONTRACTS_LOCKED.md` for crates you do NOT own.
Specifically, you MUST NOT change:
{LOCKED_INTERFACE_LIST}

## What You May Do Freely
- Add `pub(crate)` helper functions in crates you own
- Add new private modules in crates you own
- Add new `pub` items in NEW modules you create within crates you own
- Add dev-dependencies for testing

## Test Plan (Write These FIRST)
{TEST_CASES}

## Mocking Strategy
{MOCKING_INSTRUCTIONS}

## Implementation Priority
1. Write all tests above — verify they compile
2. Implement until all tests pass
3. Remove all `todo!()` and silent stubs from non-test code
4. Run `cargo clippy -p {CRATE_NAME}` — fix all warnings
5. Run `cargo fmt`

## Verification Commands
```bash
cargo test -p {CRATE_NAME}
cargo clippy -p {CRATE_NAME} -- -D warnings
cargo fmt --check -p {CRATE_NAME}
cargo check --workspace
```

## Definition of Done
- [ ] All test cases pass
- [ ] Zero `todo!()` in non-test code
- [ ] Zero clippy warnings
- [ ] `cargo fmt` applied
- [ ] `cargo check --workspace` succeeds
- [ ] PR created against `main` with branch `stream-S{ID}/v0.1.0`

## Dependencies on Other Streams
{UPSTREAM_DEPENDENCIES}

## Downstream Consumers
{WHO_USES_YOUR_OUTPUT}
