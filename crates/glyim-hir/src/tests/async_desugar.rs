//! Tests for W2-C11-§1.1: async multi-poll state-machine desugar routing.
//!
//! These tests verify the *structural* desugar decision only (the bail-out gate
//! that engages the state-machine path when `suspend_count >= 2`). They build a
//! `CrateHir` by hand (with `Expr::Await` nodes already present) and run
//! `desugar_async` directly, bypassing the source parser. This is necessary
//! because the source-level `async fn`/`.await` lower (in `lower_expr.rs`) does
//! NOT yet emit `Expr::Await` for `.await` postfixes inside async bodies — the
//! `.await` operand is dropped and `let` bindings in async bodies are omitted,
//! so a parsed `async fn` body never contains `Expr::Await`. That parser gap is
//! tracked separately in §6.1 / KNOWN_GAPS; here we test the desugar logic in
//! isolation, which is the part §1.1 owns.
//!
//! The multi-poll poll-body uses placeholder `i32` future/live-local field types
//! because HIR is pre-type-check (the suspended future's type is unknowable
//! without future-type inference); see module docs in `lower/lower_async.rs`.

use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::interner::Interner;
use glyim_span::Span;

use crate::{
    Body, BodyId, CrateHir, EnumItem, Expr, ExprId, FnItem, Item, ItemId, ItemKind, Name, Param,
    Path, PathKind, PathSegment, StructKind, TypeRef, Variant, Visibility,
};

use crate::lower::lower_async::desugar_async;

/// Build a `CrateHir` with a single `async fn two(a, b)` whose body is a block
/// containing `n_awaits` `Expr::Await` statements followed by a tail path. The
/// `Expr::Await` nodes are injected directly (the source parser would drop them).
fn build_async_hir(n_awaits: usize) -> CrateHir {
    let mut interner = Interner::new();
    let two_name: Name = interner.intern("two");
    let a_name: Name = interner.intern("a");
    let b_name: Name = interner.intern("b");
    let x_name: Name = interner.intern("x");

    let mut exprs: IndexVec<ExprId, Expr> = IndexVec::new();
    // Helper to build a bare path expression.
    let mut path_expr = |interner: &Interner, n: Name, exprs: &mut IndexVec<ExprId, Expr>| -> ExprId {
        let p = Path {
            segments: vec![PathSegment {
                name: n,
                generic_args: None,
            }],
            kind: PathKind::Plain,
        };
        exprs.push(Expr::Path(p))
    };

    let a_path = path_expr(&interner, a_name, &mut exprs);
    let b_path = path_expr(&interner, b_name, &mut exprs);
    let x_path = path_expr(&interner, x_name, &mut exprs);

    // Build `n_awaits` Await statements (each awaiting a distinct operand).
    let mut await_stmts: Vec<ExprId> = Vec::new();
    for k in 0..n_awaits {
        let operand = if k % 2 == 0 { a_path } else { b_path };
        let aw = exprs.push(Expr::Await { expr: operand });
        await_stmts.push(aw);
    }

    // Root block: first Block in the expr list is the desugar root.
    let block = exprs.push(Expr::Block {
        stmts: await_stmts,
        tail: Some(x_path),
    });
    let _ = block;

    let body = Body {
        owner: LocalDefId::from_raw(0),
        exprs: exprs.clone(),
        pats: IndexVec::new(),
        params: Vec::new(),
        span: Span::DUMMY,
        expr_spans: {
            let mut s = IndexVec::new();
            for _ in 0..exprs.len() {
                s.push(Span::DUMMY);
            }
            s
        },
    };
    let body_id = BodyId::from_raw(0);

    let params = vec![
        Param {
            name: a_name,
            ty: None,
            span: Span::DUMMY,
        },
        Param {
            name: b_name,
            ty: None,
            span: Span::DUMMY,
        },
    ];
    let fn_item = FnItem {
        params,
        return_ty: None,
        body: Some(body_id),
        is_unsafe: false,
        is_async: true,
        is_const: false,
        generic_params: Vec::new(),
        where_clauses: Vec::new(),
        abi: None,
    };
    let item = Item {
        id: ItemId::from_raw(0),
        name: two_name,
        kind: ItemKind::Fn(fn_item),
        visibility: Visibility::Inherited,
        span: Span::DUMMY,
    };

    let mut items: IndexVec<ItemId, Item> = IndexVec::new();
    items.push(item);
    let mut bodies: IndexVec<BodyId, Body> = IndexVec::new();
    bodies.push(body);
    let mut body_owners: IndexVec<BodyId, LocalDefId> = IndexVec::new();
    body_owners.push(LocalDefId::from_raw(0));

    CrateHir {
        items,
        bodies,
        body_owners,
        interner,
    }
}

/// Names of all enum items present in the HIR.
fn enum_item_names(hir: &CrateHir) -> Vec<String> {
    let mut out = Vec::new();
    for item in hir.items.iter() {
        if let ItemKind::Enum(_) = &item.kind {
            out.push(hir.interner.resolve(item.name).to_string());
        }
    }
    out
}

/// Variant names of the (first) enum whose name contains `substr`.
fn enum_variant_names(hir: &CrateHir, substr: &str) -> Option<Vec<String>> {
    for item in hir.items.iter() {
        if let ItemKind::Enum(e) = &item.kind {
            let name = hir.interner.resolve(item.name).to_string();
            if name.contains(substr) {
                return Some(
                    e.variants
                        .iter()
                        .map(|v: &Variant| hir.interner.resolve(v.name).to_string())
                        .collect(),
                );
            }
        }
    }
    None
}

/// Field names of the `Start` variant of the `*State` enum.
fn state_start_field_names(hir: &CrateHir) -> Option<Vec<String>> {
    for item in hir.items.iter() {
        if let ItemKind::Enum(e) = &item.kind {
            let name = hir.interner.resolve(item.name).to_string();
            if name.contains("State") {
                let start = e
                    .variants
                    .iter()
                    .find(|v| hir.interner.resolve(v.name) == "Start")?;
                return Some(
                    start
                        .fields
                        .iter()
                        .map(|f| hir.interner.resolve(f.name).to_string())
                        .collect(),
                );
            }
        }
    }
    None
}

/// §1.1 routing: a function with exactly ONE `.await` must NOT produce a
/// state-enum-backed future (it stays on the existing single-poll path).
#[test]
fn single_await_no_state_enum() {
    let mut hir = build_async_hir(1);
    desugar_async(&mut hir);
    let enums = enum_item_names(&hir);
    assert!(
        !enums.iter().any(|n| n.contains("State")),
        "single-await fn should not produce a *State enum; found: {:?}",
        enums
    );
}

/// §1.1 routing: a function with TWO `.await`s must produce a `*State` enum with
/// exactly the variants `Start`, `S0`, `S1`, `Done` (2 suspends => 4 variants).
#[test]
fn two_await_state_enum_has_four_variants() {
    let mut hir = build_async_hir(2);
    desugar_async(&mut hir);
    let variants = enum_variant_names(&hir, "State")
        .expect("two-await fn must produce a *State enum");
    assert_eq!(
        variants,
        vec![
            "Start".to_string(),
            "S0".to_string(),
            "S1".to_string(),
            "Done".to_string()
        ],
        "state enum variant shape mismatch: {:?}",
        variants
    );
}

/// §1.1 invariant: `Start` must capture the function's parameters (here `a`, `b`
/// are rebound as `f0`, `f1` inside `Start`).
#[test]
fn state_enum_start_captures_params() {
    let mut hir = build_async_hir(2);
    desugar_async(&mut hir);
    let field_names = state_start_field_names(&hir).expect("State enum Start variant present");
    assert_eq!(
        field_names,
        vec!["f0".to_string(), "f1".to_string()],
        "Start must capture params as f0,f1; got {:?}",
        field_names
    );
}
