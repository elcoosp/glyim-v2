# Recommended Innovative Phase: **Comptime Partial Evaluation**

You already have `glyim-mir-interp` and `ConstKind`/`MirConst` infrastructure, but no formal phase that **lifts the interpreter into the compilation pipeline** to evaluate MIR bodies at compile time and substitute results back. This is what makes Zig's `comptime` and Lean's tactics powerful — and it fits naturally between your existing `glyim-lower` → `glyim-opt` stages.

---

## Why This Phase Fits Your Architecture

Your existing infrastructure already has **every building block**:

| Existing Piece | Comptime Use |
|---|---|
| `glyim-mir-interp::Interpreter` | Execute MIR bodies at compile time |
| `MirConstKind::Int/Uint/FloatBits/Bool/String/Unit/Fn` | Represent computed results as MIR constants |
| `TyKind::Infer(TyVar/IntVar/FloatVar)` + `InferenceTable` | Resolve inference variables from interpreted values |
| `ConstKind::Int/Uint/FloatBits/Bool/Char/String/Unit` | Feed results back into the type system (array lengths, etc.) |
| `glyim-solve::FulfillmentCtx` | Discharge obligations proven by evaluation |
| `glyim-layout::LayoutComputer` | Compute layouts from comptime-determined types |
| `Substitution` + `Const` | Instantiate generic parameters with comptime values |

---

## Proposed Crate: `glyim-comptime`

### Public Contract (new items)

```rust
// ─── Configuration ───────────────────────────────────────────

/// Controls how aggressively the comptime phase evaluates.
pub struct ComptimeConfig {
    /// Maximum interpreter steps per body (prevents divergence)
    pub step_limit: usize,            // default: 1_000_000
    /// Maximum recursive comptime calls
    pub recursion_limit: usize,       // default: 128
    /// Whether to evaluate `const` fn calls in non-const contexts (speculative)
    pub speculative: bool,            // default: false
    /// Whether to evaluate const generics / array lengths
    pub eval_const_generics: bool,    // default: true
}

impl Default for ComptimeConfig { ... }

// ─── Result Types ────────────────────────────────────────────

/// What the comptime phase produces for a single body.
pub struct ComptimeEval {
    /// The original item that was evaluated.
    pub item: MonoItem,
    /// The computed constant result, if evaluation succeeded.
    pub value: Option<InterpValue>,
    /// The MIR body with comptime-known values substituted in.
    pub optimized_body: Body,
    /// Which locals/operands were proven constant.
    pub proven_constants: Vec<(LocalIdx, MirConst)>,
    /// Diagnostics from evaluation (timeouts, panics, etc.).
    pub diagnostics: Vec<GlyimDiagnostic>,
}

/// Overall result of the comptime phase.
pub struct ComptimeResult {
    /// Successfully evaluated items.
    pub evaluated: IndexVec<ComptimeItemId, ComptimeEval>,
    /// Items that were partially evaluated (some branches proven constant).
    pub partially_evaluated: Vec<LocalDefId>,
    /// Updated type context (new constants may resolve inference vars).
    pub updated_ctx: TyCtx,
    /// All diagnostics.
    pub diagnostics: Vec<GlyimDiagnostic>,
}

/// Index for comptime-evaluated items.
pub struct ComptimeItemId { from_raw, to_raw, index }

// ─── Candidate Discovery ────────────────────────────────────

/// An item that is a candidate for comptime evaluation.
pub enum ComptimeCandidate {
    /// A `const` item — always evaluated.
    Const { def_id: ConstDefId, substs: Substitution },
    /// A `static` item — evaluated for initialization.
    Static { def_id: StaticDefId },
    /// A `const fn` called with all-constant arguments.
    ConstFnCall { def_id: FnDefId, substs: Substitution, args: Vec<MirConst> },
    /// An array length expression that is const-evaluable.
    ArrayLen { ty: Ty },
    /// An enum discriminant.
    Discriminant { adt: AdtId, variant: VariantIdx },
    /// A `where` clause constant expression.
    WhereConst { def_id: LocalDefId, const_ref: ConstRef },
}

/// Discover all comptime-evaluable items in a crate.
pub fn discover_candidates(
    hir: &CrateHir,
    ctx: &TyCtx,
    mono_items: &[MonoItemData],
) -> Vec<ComptimeCandidate>;

// ─── The Main Phase Entry Point ──────────────────────────────

/// Run the comptime partial evaluation phase.
///
/// This should be called AFTER `glyim-lower` (monomorphization)
/// but BEFORE `glyim-opt`, so that comptime results can inform
/// the optimizer.
pub fn eval_comptime(
    ctx: TyCtxMut,
    candidates: &[ComptimeCandidate],
    mir_bodies: &dyn Fn(DefId, &Substitution) -> Arc<Body>,
    config: &ComptimeConfig,
) -> ComptimeResult;

// ─── Substitution / Rewriting ────────────────────────────────

/// Substitute comptime-known values back into a MIR body.
///
/// Replaces:
///   - `Operand::Copy(p)` / `Operand::Move(p)` with `Operand::Constant(v)`
///     when `p` is a proven-constant local
///   - `Rvalue::BinaryOp(op, (a, b))` with `Rvalue::Use(Operand::Constant(result))`
///     when both operands are constant
///   - `TerminatorKind::SwitchInt` with `TerminatorKind::Goto`
///     when the discriminant is constant
///   - `Rvalue::Aggregate(AggregateKind::Array(_), elems)` with
///     `Rvalue::Repeat(elem, count)` when all elems are identical constants
///   - `Rvalue::Cast(CastKind::*, op, ty)` when the operand is constant
pub fn substitute_constants(
    body: &Body,
    constants: &[(LocalIdx, MirConst)],
    ctx: &mut TyCtxMut,
) -> Body;

/// Fold a comptime-evaluated `InterpValue` back into a `MirConst`.
pub fn interp_value_to_mir_const(
    value: &InterpValue,
    ty: Ty,
    ctx: &mut TyCtxMut,
) -> Result<MirConst, GlyimDiagnostic>;

/// Fold a comptime-evaluated `InterpValue` back into a `Const`
/// (for use in the type system — array lengths, const generics).
pub fn interp_value_to_const(
    value: &InterpValue,
    ty: Ty,
    ctx: &mut TyCtxMut,
) -> Result<Const, GlyimDiagnostic>;

// ─── Const-Condition Branch Elimination ──────────────────────

/// When a `SwitchInt` has a known discriminant, eliminate dead branches
/// and mark their basic blocks as unreachable.
pub fn eliminate_dead_branches(
    body: &mut Body,
    constants: &[(LocalIdx, MirConst)],
) -> DeadBranchStats;

pub struct DeadBranchStats {
    pub branches_eliminated: usize,
    pub basic_blocks_removed: usize,
    pub locals_dead: Vec<LocalIdx>,
}

// ─── Const Generic Resolution ────────────────────────────────

/// Attempt to resolve all `ConstKind::Infer(ConstVar)` in a type
/// by finding comptime-evaluated values for the corresponding
/// const parameters.
pub fn resolve_const_infer_vars(
    ty: Ty,
    evaluated: &IndexVec<ComptimeItemId, ComptimeEval>,
    ctx: &mut TyCtxMut,
) -> Ty;

// ─── Safety: Comptime Divergence Guard ───────────────────────

/// Check whether a body is guaranteed to terminate within
/// the step limit. Uses a simple structural check:
///   - No unbounded loops (while/loop with no break on const cond)
///   - No recursive calls without a decreasing argument
///   - No function-pointer calls
pub fn check_terminates(
    body: &Body,
    step_limit: usize,
) -> TerminationGuarantee;

pub enum TerminationGuarantee {
    /// Structurally guaranteed to terminate.
    Guaranteed { max_steps: usize },
    /// Cannot prove termination — may still terminate.
    Unknown,
    /// Structurally infinite (e.g., `loop {}`).
    Infinite,
}

// ─── Comptime Assertions ────────────────────────────────────

/// Verify `const { ... }` assertions at compile time.
/// These are `const_assert!` macro expansions that become
/// `ComptimeAssert` items in HIR.
pub fn verify_comptime_assert(
    cond_body: &Body,
    interp: &mut Interpreter<'_>,
    config: &ComptimeConfig,
) -> Result<(), GlyimDiagnostic>;
```

---

## Pipeline Integration

```
Lex → Parse → Macro Expand → Def Map → HIR Lower
  → Typeck (incl. THIR) → MIR Lower → Monomorphize
  → ★ COMPTIME PARTIAL EVAL ★    ←── NEW
  → Borrowck → Optimize → Layout → Codegen
```

The key insight: **after monomorphization, all generic parameters are concrete**, so the interpreter can actually run the code. This is the perfect insertion point.

---

## What This Enables in the Language

```glyim
// 1. Const generics with real computation
fn make_buffer<const N: usize>() -> Buffer<N>
    where const { N > 0 }   // comptime assertion
{
    // N is known at compile time, so Buffer<N> has a
    // layout computed from the evaluated N
}

// 2. Compile-time string parsing
const CONFIG: Config = comptime {
    let raw = include_str!("config.toml");
    parse_toml(raw)  // evaluated at compile time, no runtime cost
};

// 3. Type-level computation
type Matrix<T, const R: usize, const C: usize> = [[T; C]; R];
type MatMul<A, B> = Matrix<f64,
    const { A::ROWS },       // comptime field access
    const { B::COLS }
> where const { A::COLS == B::ROWS };  // comptime assertion

// 4. Branch elimination on known values
fn process(flags: u32) -> u32 {
    // If the optimizer knows flags = 0x3 from comptime context,
    // the SwitchInt is replaced with Goto
    if flags & 0x1 != 0 { branch_a() } else { branch_b() }
}
```

---

## Alternative Innovative Phases (Ranked)

| Priority | Phase | Innovation | Leverages Existing |
|----------|-------|------------|-------------------|
| 🥇 | **Comptime Partial Eval** | Zig-grade compile-time execution | `mir-interp`, `Const`, `InferenceTable` |
| 🥈 | **Effect Checking** | Track IO/panic/alloc effects in types | `Predicate`, `TraitSolver`, `FnSig` |
| 🥉 | **Escape Analysis** | Stack-allocate escaping references | `MIR`, `BorrowKind`, `Place` |
| 4 | **Contract Verification** | Pre/post conditions proved at compile time | `Interpreter`, `FulfillmentCtx` |
| 5 | **Value Range Analysis** | Prove integer bounds, eliminate overflow checks | `SwitchInt`, `IntVar`, `BinOp` |

---

### 🥈 Effect Checking (if you want something more novel)

A phase that computes a **capability set** for every function and enforces that callees don't exceed callers' allowed effects:

```rust
pub enum Effect {
    Io,
    Alloc,
    Panic,
    EnvAccess,
    Ffi,
}

pub struct EffectSet(/* bitset */);

pub fn check_effects(
    hir: &CrateHir,
    mir_bodies: &dyn Fn(DefId) -> Arc<Body>,
    ctx: &TyCtx,
) -> EffectResult;
```

This would wire into your `Predicate` system as `Predicate::Effect(FunctionId, EffectSet)` and use `TraitSolver` to prove effect bounds. It's genuinely novel for a systems language but requires more design work than comptime.

---

**My recommendation: start with comptime partial evaluation.** You have all the machinery — `Interpreter`, `MirConst`, `ConstKind`, `InferenceTable` — it's a natural fit, and it unlocks the most user-facing power per line of code written.
Given your UCMS design, you have a full **Comptime Virtual Machine (CVM)** capable of executing Glyim code at compile time. However, right now, the CVM's only job is to manipulate syntax (generating `TokenStream`s via `quote!`). 

This is leaving the most powerful feature of a compile-time interpreter on the table. **The most innovative phase you can add is `glyim-contracts` (Refinement Types & Comptime Verification), which turns your CVM into a lightweight theorem prover.**

Here is why this fits perfectly, and how to design it.

---

## The Pitch: From Macro Interpreter to Theorem Prover

Systems languages like Rust and Zig have compile-time evaluation, but they only use it for code generation and constant folding. Academic languages like F*, Liquid Haskell, and Dafny use refinement types to mathematically *prove* program correctness, but they require complex external SMT solvers.

By adding a **Contract Verification Phase** that leverages your CVM, Glyim can offer Refinement Types natively. The CVM evaluates boolean predicates on constant values; if the value isn't known at compile time, the phase automatically inserts zero-cost MIR assertions for runtime checking.

### What the User Writes
```glyim
// 1. Define a refined type with a comptime predicate
type Percentage = i32 where self > 0 && self < 100;

// 2. Unified Capability/Effect Tracking
fn read_db() -> Data requires Io { ... }

// 3. The compiler proves correctness at compile time
fn main() {
    let a: Percentage = 50;       // CVM evaluates 50 > 0 && 50 < 100 -> Proven! No runtime cost.
    let b: i32 = get_input();
    let c: Percentage = b;        // Cannot prove at compile time -> Auto-inserts MIR runtime check.
}
```

---

## How It Synergizes with UCMS

1. **Unifies Comptime & Runtime Capabilities:** Your UCMS `#[comptime(capabilities = "fs, env")]` model naturally becomes a `Contract` predicate. The effect system is just type-level contract checking.
2. **Reuses CVM Snapshots:** The UCMS spec already specifies short-lived, COW `TyCtx` snapshots for macro evaluation. `glyim-contracts` uses the exact same snapshot mechanism to evaluate predicates without polluting the main inference table.
3. **Empowers Metaprogramming:** A `comptime fn` can query if a type satisfies a contract (`type_satisfies_contract(ty, contract)`) and generate hyper-optimized code paths that bypass runtime checks.

---

## Pipeline Integration

```
Lex → Parse → UCMS Macro Expand → Def Map → HIR Lower
  → Typeck (generates Contract obligations)
  → ★ CONTRACT VERIFICATION (glyim-contracts) ★   ←-- NEW
  → MIR Lower → Borrowck → Optimize → Codegen
```

*Note: It must run after `Typeck` (so types and inference variables are resolved) but before `MIR Lower` (so it can convert unproven contracts into MIR `Assert` statements).*

---

## Proposed Public Contract Additions

### 1. Extensions to `glyim-type`

```rust
// Extend the existing Predicate enum
pub enum Predicate {
    Trait(...),
    RegionOutlives(...),
    TypeOutlives(...),
    WellFormed(...),
    Coerce(...),
    /// NEW: A contract that must hold true for a value of type `Ty`.
    Contract(ContractPredicate),
}

/// Represents a comptime-verifiable boolean predicate on a type.
pub struct ContractPredicate {
    /// The `comptime fn(T) -> bool` that verifies the contract.
    pub checker: FnDefId,
    /// The type the contract applies to.
    pub ty: Ty,
    pub span: Span,
}

/// First-class representation of a contract.
pub struct ContractId — from_raw, to_raw, index;

pub struct ContractDef {
    pub name: Name,
    pub checker: FnDefId,
    /// The capabilities/effects this contract allows (unifies with UCMS capabilities)
    pub effects: EffectSet, 
    pub span: Span,
}

/// Runtime/Comptime effects, bridging UCMS capabilities to the type system.
pub enum Effect — Io, Alloc, Panic, Env, Ffi;
pub struct EffectSet(/* bitflags */);

// Extend ObligationCauseCode
pub enum ObligationCauseCode — WellFormed, TypeConstruction, MatchArm, IfThenElse, 
    ContractCast, ContractBinding;
```

### 2. Extensions to `glyim-mir`

Instead of adding new MIR statements, we elegantly reuse the existing `Assert` terminator for runtime contract checks.

```rust
// Extend existing AssertMessage
pub enum AssertMessage — Overflow(BinOp), DivisionByZero, RemainderByZero, BoundsCheck,
    /// NEW: A contract check failed at runtime.
    ContractViolation(Name);
```

### 3. New Crate: `glyim-contracts`

This phase intercepts `Predicate::Contract` obligations generated by `Typeck`.

```rust
pub struct ContractResult {
    /// Obligations proven by the CVM at compile time (zero runtime cost).
    pub proven: Vec<(Obligation, ProofKind)>,
    /// Unproven obligations that require runtime checks.
    pub inserted_checks: Vec<(BodyId, ExprId, ContractId)>,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

pub enum ProofKind {
    /// Proven by evaluating the CVM on a constant value.
    ComptimeEval(CvmValue),
    /// Proven via trait implementation (e.g., T: NonZero).
    TraitImpl,
    /// Could not be proven; runtime check inserted into MIR.
    RuntimeCheck,
}

/// Evaluate and discharge contract obligations.
/// 
/// - For constants: Invokes the CVM `checker` function. If it returns true, 
///   the obligation is discharged. If false, emits a compile error.
/// - For variables: Marks the obligation for `insert_runtime_checks`.
pub fn check_contracts(
    ctx: TyCtxMut,
    obligations: &[Obligation],
    cvm: &mut CvmExecutor, // Integration with your UCMS CVM
) -> (TyCtx, ContractResult);

/// Modifies MIR bodies to include `TerminatorKind::Assert` for contracts
/// that could not be proven at compile time.
pub fn insert_runtime_checks(
    body: &mut Body,
    checks: &[(ExprId, ContractId)],
    ctx: &TyCtx,
);
```

---

## How the CVM Evaluates a Contract (Algorithm)

When `check_contracts` encounters a `Predicate::Contract` for a cast (e.g., `let x: Percentage = 50;`):

1. **Resolve the value:** The phase asks `InferenceTable` if the value being cast is a known constant (`MirConstKind::Int(50)`).
2. **Comptime Path (Constant):** 
   - The phase takes a COW snapshot of `TyCtx`.
   - It invokes the CVM: `cvm.call(checker_fn, vec![CvmValue::Int(50)])`.
   - If CVM returns `CvmValue::Bool(true)`, the obligation is discharged. **Zero runtime cost.**
   - If CVM returns `CvmValue::Bool(false)`, emit `GlyimDiagnostic` (compile error).
3. **Runtime Path (Variable):**
   - If the value is not a constant, the phase adds it to `inserted_checks`.
   - Later, `insert_runtime_checks` modifies the MIR `Body`: it inserts a `TerminatorKind::Assert` evaluating the `checker` function, with `AssertMessage::ContractViolation("Percentage")`.

---

## Why This Makes Glyim Stand Out

Zig has `comptime`, but no automated verification or type-level effect tracking. Rust has macros and is adding effects, but lacks refinement types.

By adding `glyim-contracts`, Glyim becomes a **Verified Systems Language**. You get the safety guarantees of languages like F* or Dafny, but backed by a familiar systems-level syntax and your powerful UCMS infrastructure. The CVM does double duty: it generates code when you need syntax, and it proves correctness when you need safety.
