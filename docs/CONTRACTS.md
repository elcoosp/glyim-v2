```markdown
# Glyim Internal Interaction Contract Manifesto

**Document Version:** 1.0.0  
**Status:** Draft  
**Date:** 2025-04-10

This document defines the strict **contracts** (inputs, outputs, error handling) and **DX requirements** (latency, observability, ergonomics) between all Glyim compiler crates.

Our overriding goals are:
1.  **Debugability:** Every layer must emit structured `tracing` spans.
2.  **Ergonomics:** Errors must be `miette` diagnostics, not `panic!` or `Result::Err`.
3.  **Observability:** Internal state must be inspectable via logging, not just printf.

---

## 1. The Universal Contract: Tracing & Spans

All crates must adhere to the "Span-First" contract to enable end-to-end debugging from CLI to LLVM.

### 1.1 The Span Protocol
*   **Interface:** `tracing::Span` + `tracing::IdGenerator`.
*   **Input:** Source location (`glyim-span::Span`).
*   **Output:** A `tracing::Span` context attached to async tasks.
*   **Requirements:**
    *   All public functions taking `&ast SyntaxNode` or `Span` must accept a `tracing::Span`.
    *   **Crates:** `glyim-db`, `glyim-parse`, `glyim-typeck`, `glyim-lsp`.

### 1.2 Error Reporting Protocol
*   **Interface:** `miette` (Diagnostic Struct).
*   **Requirements:**
    *   Errors are returned via `Result<T, Vec<Diagnostic>>` or emitted to a channel.
    *   **Crates:** `glyim-diag` (definition), `glyim-parse`, `glyim-typeck`.
    *   **DX Rule:** Never hide an error. Always report a primary error and list suppressed (children) errors.

---

## 2. Frontend Contracts (Parsing & Structure)

### 2.1 Source to Tokens (`glyim-lex` -> `chumsky`)
*   **Input:** `&str` (source code).
*   **Output:** `Vec<Token>` (via `chumsky` stream).
*   **DX Requirement:** Must handle files >100MB without OOM.
*   **Debug:** Emit `lex` spans for token duration.

### 2.2 Tokens to AST (`glyim-parse` -> `rowan`)
*   **Input:** Token stream.
*   **Output:** `rowan::SyntaxNode` (Lossless Tree).
*   **DX Requirement:** "Error Tolerance" — Continue parsing if syntax is incomplete to recover the AST for IDE features.
*   **Contract:** `rowan` tree **must** contain valid trivia (whitespace) for `glyim-fmt`.

### 2.3 AST to DefMap (`glyim-parse` -> `glyim-def-map`)
*   **Input:** `rowan::SyntaxNode`.
*   **Output:** `CrateDefMap` (Item IDs, structure only, NO types).
*   **DX Requirement:** **Tier 1 Speed.** This query must execute in `<50ms` for a single file update.
*   **Optimization:** Use `lasso` for interned identifiers (Item Names) to reduce hashing overhead.

---

## 3. Database Contracts (The Salsa Heart)

### 3.1 Query Definition (`glyim-db` -> `salsa`)
*   **Input:** User triggers (File changed, LSP request).
*   **Output:** Re-computed values (AST, Types, MIR).
*   **Contract:**
    *   **Cyclic Dependencies:** `salsa` config must detect cycles and break them with a "Cycle Detected" diagnostic.
    *   **Cancellation:** All queries must inspect `tracing::current_span().is_cancelled()`.
*   **Debug:** `tracing` instrumentor on every `salsa` query execution to visualize the dependency graph in Jaeger/Zipkin.

---

## 4. Analysis Contracts (Types & Logic)

### 4.1 Type Inference (`glyim-typeck` -> `glyim-infer` + `chalk-solve`)
*   **Input:** `thir::Body` (Typed HIR Body).
*   **Output:** `InferenceResult` (Type for every expression).
*   **DX Requirement:** **Type Inference Errors.** If types mismatch, the error must point to the *expression* and show the *expected* vs *found* type.
*   **Debug:** `chalk-solve` logic steps should be emitted as debug logs via `tracing` (optional, behind `--debug` flag).

### 4.2 THIR Generation (`glyim-typeck` -> `glyim-thir`)
*   **Input:** `hir::Body`.
*   **Output:** `thir::Body` (Explicit generic parameters).
*   **Contract:** THIR is the boundary between "Generic" and "Concrete".

### 4.3 MIR Lowering (`glyim-lower` -> `glyim-mir`)
*   **Input:** `thir::Body` (Monomorphized).
*   **Output:** `mir::Body` (CFG based).
*   **DX Requirement:** "Borrowck visibility." The MIR must clearly label which variables are live on which edges for the borrow checker.
*   **Optimization:** Use `fixedbitset` for Liveness Analysis in the borrow checker.

### 4.4 Comptime Execution (`glyim-mir-interp` -> `regalloc2`)
*   **Input:** `mir::Body`.
*   **Output:** `Value` (Comptime result) or `Injection` (New AST nodes).
*   **Contract:**
    *   **Interpreter Loop:** Must detect infinite loops in comptime code and abort with a `ComptimeTimeout` diagnostic.
    *   **Safety:** Interpreter must perform bounds checks; panics here are compiler bugs, not user errors.

---

## 5. Optimization Contracts (E-Graphs)

### 5.1 E-Graph Rewriting (`glyim-egraph` -> `egg`)
*   **Input:** `mir::Body`.
*   **Output:** Optimized `mir::Body`.
*   **DX Requirement:** "Optimization Reporting." If a loop is vectorized, emit a note: "Optimized loop in function X."
*   **Debug:** Serialize the E-Graph to DOT format on failure for inspection.

---

## 6. Backend Contracts (Codegen)

### 6.1 MIR to LLVM (`glyim-codegen-llvm` -> `inkwell` + `llvm-sys`)
*   **Input:** `mir::Body` + `regalloc2` results (Allocations).
*   **Output:** LLVM Module (`inkwell::Module`).
*   **Contract:**
    *   **ABI Stability:** Generated LLVM IR must be deterministic (same inputs -> same bitstream) for reliable caching in `glyim-cas`.
    *   **Verify Mode:** If `--verify-llvm` is set, run the LLVM verifier on the module and report errors as Glyim diagnostics.

---

## 7. Tooling Contracts (Interface)

### 7.1 LSP <-> Core (`glyim-lsp` -> `glyim-db`)
*   **Transport:** `async_lsp` + `tokio`.
*   **Protocol:** JSON-RPC (Standard LSP).
*   **DX Requirement:** **Cancellation.** If a user types a new character, pending type-checking requests for the old file must be cancelled immediately via `tokio::select!`.
*   **Streaming Errors:** `miette` diagnostics must be pushed to the client as they are discovered, not buffered to the end of compilation.

### 7.2 CLI <-> Core (`glyim-cli` -> `glyim-db`)
*   **Input:** `clap` arguments.
*   **Output:** Stdout (Human readable) or Exit Code.
*   **DX Requirement:** **Color & Terminal UI.** Use `owo-colors` + `miette` for beautiful terminal output.
*   **Contract:** CLI acts as a orchestrator. It should not contain compiler logic, only logic to invoke `glyim-db` and render `miette` results.

---

## 8. Summary of Critical Data Flows (The "Happy Path")

1.  **File Load:** VFS -> `glyim-lex` -> `chumsky` -> `glyim-parse` -> `rowan` (Span attached).
2.  **Fast Path (IDE):** `rowan` -> `glyim-def-map` -> Store in `salsa` (<50ms).
3.  **Slow Path (Build):** `salsa` -> `glyim-typeck` -> `chalk-solve` -> `glyim-thir`.
4.  **Optimization:** `glyim-thir` -> `glyim-mono` -> `glyim-mir` -> `glyim-egraph`.
5.  **Codegen:** `glyim-egraph` -> `glyim-codegen-llvm` -> Object File.

---
```
