## Parallel Flow for Unstubbing (U01–U08)

Based on the dependency graph defined in your `streams.json`, the unstubbing streams can be executed in **three parallel waves**:

### Wave 0 – Independent starting points (can run simultaneously)

| Stream | Crate | Upstream | Estimated effort |
|--------|-------|----------|------------------|
| **U01** | `glyim-typeck` | none | large |
| **U06** | `glyim-frontend` (parser) | none | medium |

These two streams modify completely different crates and have no mutual dependencies. They can be assigned to two different agents right away.

### Wave 1 – After U01 and U06 are complete

Once **U01** finishes, **U02** can start.  
Once **U06** finishes, **U07** and **U08** can start.

| Stream | Crate | Depends on | Can start when |
|--------|-------|------------|----------------|
| **U02** | `glyim-lower` | U01 | U01 merged |
| **U07** | `glyim-hir` | U06 | U06 merged |
| **U08** | `glyim-def-map` | U06 | U06 merged |

**U07** and **U08** can run in parallel (different crates). **U02** runs alone (no other stream depends on it yet).

### Wave 2 – After U02 finishes

Once **U02** is merged, three streams can start in **full parallelism** because they touch different crates and have no further dependencies on each other:

| Stream | Crate | Depends on |
|--------|-------|-------------|
| **U03** | `glyim-mir-interp` | U02 |
| **U04** | `glyim-codegen` (bytecode) | U02 |
| **U05** | `glyim-pipeline` (ADT support) | U02 |

These can be assigned to three different agents simultaneously.

### Visual Timeline (assuming 1 unit = 1 agent‑week)

```
Week 1-2:   U01 ─────────────────────┐
            U06 ───────┐              │
Week 3-4:              ├── U07 ───────┤
                       ├── U08 ───────┤
                       └── U02 ───────┼── U03 ────┐
                                     ├── U04 ────┤
                                     └── U05 ────┘
```

**Peak parallelism:** 4 agents simultaneously (U01, U06, U07, U08 at the same time, or later U03, U04, U05 together with something else).

### Prerequisites for Parallel Execution

- Each agent works in its own **git worktree** (`../glyim-worktrees/stream-UXX/`).
- Streams with `upstream` dependencies **must wait** until the upstream stream’s PR is merged into `main`.
- No cross‑stream file conflicts because each owns distinct crates:
  - U01 → `glyim-typeck`
  - U06 → `glyim-frontend` (parser only)
  - U07 → `glyim-hir`
  - U08 → `glyim-def-map`
  - U02 → `glyim-lower`
  - U03 → `glyim-mir-interp`
  - U04 → `glyim-codegen`
  - U05 → `glyim-pipeline`

This design allows you to keep **every agent busy** at all times, with minimal waiting.
