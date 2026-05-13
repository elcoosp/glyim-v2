# Glyim

> The "Tier 1" systems programming language unifying Pythonic ergonomics, Haskell's type theory, and Rust's memory safety.

---

**[Status:** 🏗️ Early Development **|** **Version:** 0.1.0 **|** **Rust:** 2024 Edition]

## 🚀 Vision

Glyim is designed to solve the "Systems Trilemma":
1.  **Invisible Safety:** Memory safety via an affine type system that doesn't hinder the "scripting feel."
2.  **State-of-the-Art Tooling:** IDE support (LSP) that feels instantaneous, powered by a tiered incremental compilation model.
3.  **First-Class Metaprogramming:** Compile-time execution is not a preprocessor; it is an integral part of the language.

**The Elevator Pitch:**
For high-performance developers dissatisfied with Rust's borrow checker friction or C++'s lack of safety, Glyim provides a "Tier 1" platform. It offers Python-like syntax with Haskell-level type inference and Rust-level performance, featuring a query-based compiler that keeps your tooling responsive even at massive scale.

## 🏗️ Architecture

Glyim is built as a highly modular, multi-crate workspace using **Salsa** for incremental compilation.

### The Stack
*   **Parser:** `chumsky` + `rowan` (Lossless Syntax Trees)
*   **Query Engine:** `salsa` (Incremental recomputation)
*   **Type System:** `chalk` (Trait solving) + Custom Inference
*   **Optimization:** `egg` (Equality Saturation E-Graphs)
*   **Backend:** `inkwell` (LLVM Wrapper)

### High-Level Layers
1.  **Foundation:** Interning, Spans, VFS, Database.
2.  **Frontend:** Lexing, Parsing, Definition Mapping.
3.  **Analysis:** Type Checking, Trait Solving, THIR generation.
4.  **Lowering:** MIR, Borrow Checking, E-Graph Optimization.
5.  **Backend:** LLVM Codegen, Bytecode, Runtime.

> 📚 **Deep Dive:** See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for detailed diagrams and ADRs.

## 🛠️ Development Strategy

Since we are starting from a blank page, the codebase is decomposed into **Parallel Tracks** that can be developed simultaneously. We ensure smooth integration by strictly adhering to the **Contracts** defined in [`docs/CONTRACTS.md`](docs/CONTRACTS.md) (using `miette` for errors and `tracing` for spans) and using the `glyim-db` (Salsa) as the central merging point.

### The Parallel Plan

We define 5 Epics. Teams can work on these in parallel as long as "Interface-First" contracts are honored.

#### 📍 Epic 1: The Foundation (Infrastructure)
*Focus: establishing the query engine and core data structures.*

*   **Crates:** `glyim-interner`, `glyim-span`, `glyim-db`, `glyim-vfs`.
*   **Parallel Dependency:** None (Must start first).
*   **Deliverables:**
    *   A functional `salsa` database in `glyim-db`.
    *   `Span` and `Symbol` types defined in `glyim-span` and `glyim-interner`.
    *   A virtual file system abstraction in `glyim-vfs`.
*   **Integration Point:** All other crates depend on these. Define the API surface before implementation.

#### 📍 Epic 2: The Frontend (Syntax & IDE)
*Focus: Fast, error-tolerant parsing for the LSP.*

*   **Crates:** `glyim-lex`, `glyim-parse`, `glyim-def-map`, `glyim-diag`.
*   **Parallel Dependency:** Depends on **Epic 1**.
*   **Deliverables:**
    *   `chumsky` lexer implementation.
    *   `rowan` Green Tree parser.
    *   `glyim-def-map`: A fast "Tier 1" query that builds the module tree without type checking bodies.
    *   `glyim-diag`: Unified error reporting setup.
*   **Integration Point:** `glyim-db` will expose queries `parse()` and `def_map()`.

#### 📍 Epic 3: Type System & Logic
*Focus: Hindley-Milner inference + Trait Solving.*

*   **Crates:** `glyim-infer`, `glyim-solve`, `glyim-typeck`, `glyim-thir`.
*   **Parallel Dependency:** Depends on **Epic 1**. Can mock AST inputs from Epic 2.
*   **Deliverables:**
    *   Integration with `chalk` for trait resolution in `glyim-solve`.
    *   Type inference engine in `glyim-infer`.
    *   Construction of Typed High-level IR (THIR) in `glyim-thir`.
*   **Integration Point:** `glyim-typeck` consumes the AST from Epic 2 and outputs THIR to the Database.

#### 📍 Epic 4: Lowering & Optimization
*Focus: MIR, Control Flow, and E-Graphs.*

*   **Crates:** `glyim-mir`, `glyim-lower`, `glyim-lifetime`, `glyim-borrowck`, `glyim-egraph`.
*   **Parallel Dependency:** Depends on **Epic 3** (THIR input).
*   **Deliverables:**
    *   THIR $\to$ MIR lowering.
    *   Polonius-style borrow checker (`glyim-borrowck`).
    *   `egg` based optimizer using equality saturation.
*   **Integration Point:** `glyim-mono` (monomorphization) sits here, bridging generic THIR to concrete MIR.

#### 📍 Epic 5: Backend & Tooling
*Focus: Codegen, CLI, and LSP Server.*

*   **Crates:** `glyim-codegen-llvm`, `glyim-codegen`, `glyim-cli`, `glyim-lsp`, `glyim-runtime`.
*   **Parallel Dependency:** Depends on **Epic 4** (for optimized MIR) AND **Epic 1** (for DB access).
*   **Deliverables:**
    *   LLVM IR emission via `inkwell`.
    *   Standard library runtime (`glyim-runtime`).
    *   CLI driver (`glyim-cli`).
    *   Async LSP server (`glyim-lsp`).

### Integration Strategy: "Skeletons First"

To ensure smooth merging of these parallel tracks:

1.  **Step 1 (Skeletons):** All teams define the `pub struct` and `pub fn` signatures for their crates immediately.
2.  **Step 2 (Stubs):** Implement functions returning `unimplemented!()` but ensuring they compile.
3.  **Step 3 (Wiring):** `glyim-db` is updated to call these functions as Salsa queries. This forces the crate interfaces to settle early.
4.  **Step 4 (Implementation):** Teams fill in the logic. The workspace compiles at every commit.

## 📦 Building

We use `just` as a command runner.

```bash
# Install Just if you don't have it
cargo install just

# Run the watcher (builds on file change)
just wr
```

Or standard cargo:

```bash
# Build the whole workspace
cargo build

# Run tests
cargo test

# Check formatting
cargo fmt --check
```

## 📚 Documentation

*   **[Architecture Specification](docs/ARCHITECTURE.md):** Deep dive into components, IRs, and design decisions.
*   **[Internal Contracts](docs/CONTRACTS.md):** Rules for error handling, tracing, and crate interaction.
*   **[Tech Stack](docs/TECHSTACK.md):** Detailed rationale for external dependencies.
*   **[Vision & Spec v0.1.0](docs/specs/v0.1.0.md):** Strategic goals and success metrics.

## 🤝 Contributing

We are looking for contributors who want to build a next-gen compiler.
1.  Check the **Parallel Plan** above to see where you can jump in.
2.  Read [`docs/CONTRACTS.md`](docs/CONTRACTS.md) before writing code.
3.  Ensure all `tracing` spans are present and `miette` diagnostics are used.

## 📄 License

[License TBD]
