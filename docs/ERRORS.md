```markdown
# Glyim Compiler Error Handling Specification

**Document Version:** 1.0.0  
**Status:** Draft  
**Date:** 2025-04-10  
**Owner:** Language Design Team

---

## 1. Philosophy & Goals

Glyim aims to provide an industry-leading Developer Experience (DX). Error handling is not just about crashing; it is the primary interface between the compiler and the user.

Our core principles for error handling are:
1.  **Clarity > Brevity:** A long, explanation-rich error is better than a cryptic "Type mismatch."
2.  **Recovery is King:** The compiler should never stop at the first error if it can continue to find more (Batch Mode). In IDE mode, it should stop on the *first fatal* error to save latency.
3.  **Traceability is Mandatory:** Every error must be pinned to a `tracing::Span` so it can be highlighted in the editor.
4.  **Structured, not Stringly:** We use `miette` for rich diagnostics and `tracing` for instrumentation. We do not rely on `String` or `anyhow` for internal error paths.

---

## 2. The Error Taxonomy

We classify errors into three distinct categories that dictate how they are handled.

### 2.1 User Errors (Diagnostic)
Errors caused by the user's code. These are mapped to `miette::Diagnostic` and sent to the client.

*   **Syntax Errors:** Lexical or grammatical issues (e.g., "Unexpected token `{`").
*   **Analysis Errors:** Type mismatches, unresolved names, trait violations.
*   **Borrow Check Errors:** Lifetime conflicts, illegal moves.
*   **Comptime Errors:** Panics or logic errors occurring in `comptime` functions.

### 2.2 ICE (Internal Compiler Errors)
Errors caused by bugs in the compiler itself (Logic bugs, `unwrap()` calls on internal state, unexpected crashes).

*   **Severity:** **Fatal**.
*   **Handling:** These panic the compiler process. We do not try to recover from ICEs. The error message must ask the user to file a bug report.
*   **Format:** "Internal Compiler Error: Please report this bug. Trace: ..."

### 2.3 Infrastructure Errors
Errors caused by the environment (File I/O, OS limits, Network timeouts for remote builds).

*   **Severity:** **Fatal** (usually).
*   **Handling:** Rendered cleanly, but terminates the operation.

---

## 3. The Diagnostic Model (`miette`)

We use `miette` as the structured format for all user-facing errors.

### 3.1 Diagnostic Structure
Every diagnostic consists of:
*   **Severity:** `Error`, `Warning`, `Help`.
*   **Code:** Machine-readable code (e.g., `E0308`, `B0001`).
*   **Message:** Human-readable primary message (e.g., "Type mismatch").
*   **Labels:** Key-value pairs for filtering (e.g., `Category="Type"`, `Span="main.gy:10:5"`).
*   **Related Spans:** A list of locations involved in the error (Primary + Secondary).
*   **Suggestions:** "Fix-it" hints (e.g., "Consider changing `x` to `&mut x`").
*   **Notes:** Extended help text (Markdown supported).

### 3.2 Severity Levels
| Level | Behavior | Example |
| :--- | :--- | :--- |
| **Error** | Stops compilation (in Build mode) or highlights only this error (in IDE mode). | "Type `i32` not found." |
| **Warning** | Does not stop compilation. | "Unused variable `x`." |
| **Note** | Purely informational. | "Optimized loop `foo`." |

---

## 4. Phase-Specific Error Handling Strategies

### 4.1 Frontend (Lexing & Parsing)
**Goal:** Maximize recovery to provide as many errors as possible.

*   **Strategy:**
    *   **Lexer:** If invalid UTF-8 is encountered, emit an error but skip the invalid byte to continue tokenizing the rest of the file.
    *   **Parser:** Use `chumsky`'s error recovery capabilities. Do not stop at the first missing brace. Build a best-effort AST to feed the analyzer.
*   **Implementation:** The parser returns `Result<AST, Vec<Diagnostic>>`. The `Vec<Diagnostic>` is accumulated into the session.

### 4.2 Analysis (Type Checking & Borrowing)
**Goal:** Precision and Ergonomics.

*   **Type Errors:**
    *   **Mismatches:** Show the expected type vs. the found type.
    *   **Inference Failure:** "Type cannot be inferred due to conflicting constraints."
    *   **Trait Solver:** If a trait is not found, provide a list of *similar* traits (Levenshtein distance < 3).
*   **Borrow Checker (The "Invisible" Goal):**
    *   **Problem:** Rust's borrow errors are notoriously hard to read ("borrow of moved value").
    *   **Solution:** We use a **"Flow Narrative"** approach. Instead of just saying "borrow error," the error explains the control flow:
        > "Value `data` is moved into `process` at line 10. However, `data` is still borrowed by `logger` at line 15."

### 4.3 Metaprogramming (Comptime)
**Goal:** Distinguish user logic errors from compiler bugs.

*   **User Panic:** If a `comptime` function panics (e.g., `assert!(false)` or `unwrap()`), it is treated as a standard user error.
    *   **Trace:** We show a stack trace of the **Glyim code** (MIR frames), not the interpreter's internal stack.
*   **Interpreter Bug:** If the `glyim-mir-interp` panics due to a compiler bug, it is treated as an **ICE** (Internal Compiler Error).

---

## 5. The Internal Error Type (`GlyimError`)

Internal crates use a dedicated enum `GlyimError` to represent failures.

```rust
pub enum GlyimError {
    /// A standard miette diagnostic.
    Diagnostic(Box<miette::Diagnostic>),

    /// Internal Compiler Error (Bug).
    ICE { 
        message: String, 
        file: String, 
        line: u32 
    },

    /// IO/Os Error.
    Io(std::io::Error),

    /// Cancellation (requested by LSP/CLI).
    Cancelled,
}
```

### 5.1 Conversion to `Result`
Most compiler functions return `Result<T, GlyimError>`.

*   **Propagation:** `?` operator is used to bubble errors up.
*   **Mapping:** `std::io::Error` is mapped to `GlyimError::Io`.
*   **Context:** When converting to `GlyimError::Diagnostic`, we use `.context()` to attach the relevant `tracing::Span`.

---

## 6. Error Emission & Rendering

### 6.1 The Emitter
We define a central `Emitter` trait in `glyim-diag`.

```rust
pub trait Emitter {
    fn emit(&mut self, diag: Diagnostic);
    fn has_errors(&self) -> bool;
}
```

*   **CLI Emitter:** Prints to `stderr` using `miette-term`. Supports color (via `owo-colors`).
*   **LSP Emitter:** Pushes JSON-RPC notifications to the client.
*   **Sarif Reporter:** Emits `sarif` files for CI/CD integration.

### 6.2 Error Rendering (Human Readable)
We prioritize **Multi-line, colored output** using `miette-term`.

*   **Format:**
    ```text
    error[E0308]: Type mismatch
      --> src/main.gy:12:15
       |
    11 |     fn add(a: i32, b: i32) -> i32 {
       |            ---^^^^^^ expected `i32`, found `bool`
    12 |         a + b
       |              --------- this expression has type `bool`
    13 |     }
    ```

---

## 7. Handling Panics (The Safety Net)

While we strive for `Result`-based logic, crashes can happen. We use a **Global Panic Hook** to format crashes nicely.

1.  **Detection:** The hook catches a panic.
2.  **Format:** Generates a generic "Internal Compiler Error" message.
3.  **Context:** Asks the user to run with `GLYIM_BACKTRACE=1`.
4.  **Exit:** Exits with code 101 (to distinguish from normal failure).

---

## 8. Integration with Tracing

Every `miette::Diagnostic` **must** contain a valid `tracing::Span`.

*   **Source Mapping:** `glyim-db` ensures that `Span` is translated into `Line` and `Column` indices relative to the file.
*   **Hygiene:** `SyntaxContext` in the span helps the LSP distinguish between the macro definition site and the call site.

---

## 9. The "Invisible" Borrow Checker Manifesto

To ensure the borrow checker feels "invisible" (helpful rather than hostile), we enforce strict output guidelines for `glyim-borrowck`.

1.  **Explain the Conflict:** Do not just say "cannot borrow `x` as mutable." Explain *why* (it is borrowed by `y` in function `foo`).
2.  **Suggest the Fix:** Always suggest a concrete code change (e.g., "Consider cloning `x` here").
3.  **Group Errors:** If 20 borrow errors exist in one function, group them into a single diagnostic with a list of violations, rather than spamming the console.

---

## 10. Cancellation Handling (LSP Priority)

In `glyim-lsp`, speed is priority.

*   **Mechanism:** `tokio::select!` listens for cancellation signals from the client.
*   **Propagation:**
    *   `salsa` queries check `tracing::current_span().is_cancelled()`.
    *   If cancelled, the query returns `Err(GlyimError::Cancelled)`.
    *   This error bubbles up, triggering the `Emitter` to stop processing immediately.

---
```
