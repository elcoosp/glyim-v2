# Glyim Agent Dispatch Kit

This directory contains the complete system for dispatching parallel LLM agents to implement the Glyim compiler. You create the files here once, then generate stream briefs and paste them into agent chats.

## File Inventory

| File | Purpose |
|------|---------|
| `README.md` | This file. Instructions for the human operator. |
| `AGENT_MASTER_CONTEXT.md` | Project-wide rules included in every agent's system prompt. |
| `CONTRACTS_LOCKED.md` | Auto-generated list of `pub` interfaces agents cannot modify. |
| `GLYIM_TEST_INSTRUCTIONS.md` | How to use `glyim-test`, write unit tests in `src/tests/`, and mock correctly. |
| `agent-prompt-template.md` | The exact prompt template to copy-paste into the LLM. |
| `stream-template.md` | Template for individual stream briefs. |
| `generate-stream.sh` | Script to generate a brief from `streams.json` using the template. |
| `streams.json` | Single source of truth for all 20 streams (scope, tests, dependencies). |
| `briefs/` | Generated per-stream markdown files (e.g., `briefs/S01.md`). |

## Setup

Ensure `jq` is installed (required by `generate-stream.sh`):

```bash
sudo apt-get install jq
```

## Generating Stream Briefs

Generate a single brief:

```bash
./docs/agent-kit/generate-stream.sh S01
```

Generate all 20 briefs:

```bash
for id in S01 S02 S03 S04 S05 S06 S07 S08 S09 S10 S11 S12 S13 S14 S15 S16 S17 S18 S19 S20; do
    ./docs/agent-kit/generate-stream.sh "$id"
done
```

Generated files appear in `docs/agent-kit/briefs/SXX.md`.

## Dispatching an Agent

For each stream, open a new chat with your LLM agent and provide exactly these documents:

### System Prompt

Paste the contents of:
1. `AGENT_MASTER_CONTEXT.md`
2. `CONTRACTS_LOCKED.md`
3. `GLYIM_TEST_INSTRUCTIONS.md`

### User Prompt

Use the template from `agent-prompt-template.md`. Fill in:
- `{ID}` with the stream ID (e.g., `S01`)
- `{NAME}` with the stream name (e.g., `Lexer`)
- Paste the contents of `briefs/S01.md`
- Paste relevant source code files the agent needs context for

### Source Code Context

Attach the source files the agent needs:
- Their owned crate's `src/lib.rs` and existing modules
- `glyim-test/src/lib.rs` (if they need test utilities)
- Relevant `glyim-type/src/*.rs` files (for type system contracts)
- Any upstream crate sources they depend on

**Do NOT attach the entire codebase.** Only attach what they need to avoid context window overflow.

## Wave Execution Order

Agents have dependencies. Execute waves in order:

```
Wave 1 (no dependencies):
  S01-Lexer, S02-TypeInterning, S03-MIRCore, S04-LayoutEngine,
  S05-Unification, S06-MIRInterpreter, S07-BytecodeBackend, S08-LLVMBackend

Wave 2 (depends on Wave 1):
  S09-Parser (needs S01)
  S10-TypeDisplay (needs S02)
  S11-TraitSolver (standalone)
  S12-HIRLowering (needs S09)

Wave 3 (depends on Wave 2):
  S13-DefMap (needs S09)
  S14-TypeckDriver (needs S05, S11, S12, S13)
  S15-MIRLowering (needs S14)
  S16-Borrowck (needs S03)
  S17-MIROpt (needs S03)

Wave 4 (depends on Wave 3):
  S18-Pipeline (needs ALL)
  S19-LSP (needs S18)
  S20-CLI (needs S18)
```

Within a wave, streams can be executed in parallel.

## Merging Agent Output

Agents produce bash scripts that write/patch files:

1. Save each fenced bash block as a `.sh` file
2. Execute them one at a time in order
3. If a script exits non-zero, paste the terminal output back to the agent for a surgical fix
4. After successful execution, the agent's changes are committed automatically

## Updating Streams

To modify stream scope, tests, or dependencies:

1. Edit `streams.json` (single source of truth)
2. Regenerate affected briefs: `./docs/agent-kit/generate-stream.sh SXX`
3. Re-dispatch the agent with the updated brief

Never edit `briefs/SXX.md` directly — it will be overwritten by the generator.
