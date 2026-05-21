## Merger Overview – What to Merge and When

Based on the tasks above, here is the recommended **merge order** across waves. Each wave represents a set of independent features that can be developed in parallel, but **merged sequentially** to avoid breaking the main branch.

---

### **Wave 1 (Foundation)** – Merge these together after all pass CI
- **S14**: Builtin macros (`file!`, `line!`, `env!`, `include!`, `concat!`, `stringify!`)
- **S15**: MIR interpreter – complete binary/bitwise ops, discriminant, len, aggregate writes

✅ **Verification**:  
- All macro expansion tests pass (`cargo test -p glyim-meta`)  
- Interpreter runs all `glyim-mir-interp` tests without stubs  
- No `tracing::warn!("STUB: ...")` remains in these crates

---

### **Wave 2 (Pattern & Expression Completeness)** – Merge after Wave 1
- **S16**: Pattern matching (or‑patterns, guards, ranges, slices)  
- **S17**: Expression completion (struct literals, `?` operator, async/await stubs)

✅ **Verification**:  
- `cargo test -p glyim-lower` passes all pattern‑related tests  
- `cargo test -p glyim-typeck` passes struct literal and range tests  
- Existing tests (e.g., `for` loops, `if let`) still pass

---

### **Wave 3 (Runtime – Environment & Filesystem)** – Merge after Wave 2
- **S18**: Runtime – environment variables, process spawning, args  
- **S19**: Runtime – file system operations (open, read, write, metadata, directories)

✅ **Verification**:  
- `cargo test -p glyim-runtime` passes all `extern "C"` FFI tests  
- A simple Glyim program using `std::fs::read_to_string` compiles and runs correctly  
- No memory leaks or double frees in runtime (run under valgrind/ASAN if possible)

---

### **Wave 4 (Networking, Threading & LSP)** – Merge after Wave 3 (can be parallelised, but merge together)
- **S20**: Runtime – TCP/UDP networking, threading, time  
- **S21**: LSP – incremental analysis and symbol indexing  
- **S22**: LSP – reference graph and cross‑file navigation

✅ **Verification**:  
- Networking tests pass locally (loopback)  
- Thread tests spawn and join correctly  
- LSP: symbols index builds, `goto_definition` works across files, `find_references` returns correct locations  
- `cargo test -p glyim-lsp` passes

---

### **Wave 5 (Build Tool – glyip)** – Merge after Wave 4
- **S23**: `glyip` – dependency resolution, fingerprint caching, build/test/run commands

✅ **Verification**:  
- `glyip new` creates a valid project  
- `glyip build` compiles a simple binary incrementally  
- `glyip test` discovers and runs tests  
- `glyip run` executes binary  
- Fingerprint cache prevents unnecessary recompilation

---

### **Wave 6 (AI Pilot – Full Automation)** – Merge last, after everything above
- **S24**: `glyim-pilot` – all gates (architecture, banned patterns, contracts, coverage, mutation, self‑review) and orchestrator

✅ **Verification**:  
- `glyim-pilot serve` starts WebSocket server  
- Extension connects and sends `::WRITE`/`::COMMIT` blocks  
- Architecture gate blocks forbidden dependencies  
- Contract gate detects locked interface removal  
- Coverage gate fails when below threshold  
- Orchestrator commits after all gates pass  
- Rate limiting and failover work as expected

---

## **Important Merge Rules**

1. **Never merge a wave if any of its tasks have failing tests**  
   - Each task must have its own `SXX-TXX` tests passing before the wave merge.

2. **Merge the entire wave as a single PR** (or multiple tightly‑coupled PRs that are all reviewed together)  
   - This avoids partial feature flags and keeps `main` always in a working state.

3. **After each wave merge, run the full test suite** (`cargo test --workspace`) to catch any cross‑wave regressions.

4. **If a later wave depends on an earlier wave, the earlier wave must be fully merged before starting work on the later wave.**  
   - Example: Wave 2 depends on Wave 1’s interpreter (for pattern match testing via interpretation).  
   - Example: Wave 4 LSP depends on Wave 2’s complete HIR/THIR.

5. **Wave 3, 4, and 5 can be developed in parallel by different team members, but merges must follow the order** (3 → 4 → 5) because Wave 4 tests may use the runtime from Wave 3.

---

## **Quick Reference – Merge Timeline (example)**

| Week | Merge Wave | Focus |
|------|-----------|-------|
| 1    | Wave 1    | Builtin macros + interpreter completeness |
| 2    | Wave 2    | Pattern matching + expression completion |
| 3    | Wave 3    | Runtime env + filesystem |
| 4    | Wave 4    | Networking, threading, LSP core |
| 5    | Wave 5    | Build tool `glyip` |
| 6    | Wave 6    | AI Pilot full automation |

After Wave 6, the compiler should be fully unstubbed and produce correct output for all planned features.
