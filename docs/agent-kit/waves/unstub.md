## Sequential Summary – Unstubbing Stream Dispatch Plan

All streams below are independent **except** where explicitly noted.  
You dispatch a stream by running `./docs/agent-kit/dispatch.sh UXX` and pasting the output into an agent chat.

### Dependencies Graph

```
U01 (HIR)
  └── U02 (MIR Lowering)
        ├── U03 (MIR Interpreter)
        ├── U04 (Bytecode Backend)
        ├── U05 (LLVM Core Types)
        │     └── U06 (LLVM Operators)
        └── U07 (LSP Core)
              └── U08 (LSP Advanced)
```

### Wave Workflow

**Wave 1 – Single stream (Week 1, day 1–3)**  
- Dispatch **U01** only.  
- Wait for its PR to be merged before starting Wave 2.

**Wave 2 – Single stream (after U01 merged, day 4–6)**  
- Dispatch **U02** only.  
- Wait for its PR to be merged before starting Wave 3.

**Wave 3 – Parallel (4 streams, after U02 merged, day 7–12)**  
Dispatch **all four** at the same time. They do not conflict with each other.  
- **U03** (MIR Interpreter)  
- **U04** (Bytecode Backend)  
- **U05** (LLVM Core Types)  
- **U07** (LSP Core)  

These can run in parallel because they modify different crates and have no cross‑dependencies.

**Wave 4 – Parallel (2 streams, after U05 and U07 merged, day 13–16)**  
Dispatch **U06** and **U08** at the same time.  
- **U06** depends on U05 (already merged)  
- **U08** depends on U07 (already merged)  

They modify different crates (`glyim-codegen-llvm` vs `glyim-lsp`), so they can run in parallel.

---

## Dispatching Checklist

| Wave | Streams | Parallel? | Condition |
|------|---------|-----------|------------|
| 1    | U01     | No        | – |
| 2    | U02     | No        | U01 merged |
| 3    | U03, U04, U05, U07 | Yes (4 agents) | U02 merged |
| 4    | U06, U08 | Yes (2 agents) | U05 and U07 merged |

### Estimated Timeline (with 5 agents)

- **Day 1–3:** U01 → merge
- **Day 4–6:** U02 → merge
- **Day 7–12:** U03, U04, U05, U07 in parallel (each agent takes one) → merge as they finish
- **Day 13–16:** U06 & U08 in parallel → merge

**Total:** ~3 weeks of agent work, but parallel execution reduces wall‑clock time to ~16 days.

After all streams are merged, run the final stub check:

```bash
grep -rn "STUB:" crates/ --include="*.rs" | grep -v "compile_error"
```

It should return **nothing** (except the intentional `compile_error!` macro in `glyim-diag`).
