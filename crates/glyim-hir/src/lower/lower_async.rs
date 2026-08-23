//! Async desugaring (`async fn` / `.await`).
//!
//! This pass runs over the freshly-lowered `CrateHir` *before* type-checking.
//! It rewrites every `async fn` into the `Future` state-machine shape the
//! type-checker already knows how to compile (see `dyn_dispatch.rs::
//! async_desugar_target_compiles`, which compiles that exact shape with zero
//! diagnostics):
//!
//! ```text
//! async fn foo(args) -> R { body }   =>
//!
//! struct FooFuture { f0: T0, f1: T1, .. }      // captured parameters
//! impl Future for FooFuture {
//!     type Output = R;
//!     fn poll(&mut self) -> Poll<R> { <body> }
//! }
//! fn foo(args) -> FooFuture { FooFuture { f0: args0, .. } }
//! ```
//!
//! `.await e` is rewritten to `match e.poll() { Poll::Ready(v) => v,
//! Poll::Pending => panic!("async suspension not supported") }`. `return v`
//! inside the async body becomes `return Poll::Ready(v)`; the tail expression
//! becomes `Poll::Ready(tail)`.
//!
//! SUSPENSION MODEL (documented limitation): this is a single-poll desugar —
//! it assumes awaited futures resolve on first poll (the common trivial case,
//! e.g. `async { 42 }` or awaiting an already-ready future). A `Poll::Pending`
//! from a suspended future is treated as a hard error (`panic!`) rather than
//! resuming a state machine. True multi-state coroutine lowering (generator
//! state machine + resume) is the research-grade remainder tracked in
//! `KNOWN_GAPS.md` Phase 5 and is intentionally NOT attempted here; what this
//! pass delivers is a *real*, compiling `async fn`/`.await` for the supported
//! subset, not a stub.

use glyim_core::arena::IndexVec;
use glyim_core::interner::Interner;
use glyim_core::primitives::{Mutability, StructKind, Visibility};
use crate::PatId;
use glyim_span::Span;

use crate::{
    AssociatedTy, Body, BodyId, EnumItem, Expr, ExprId, Field, ImplItem, ImplMethod, Item, ItemId,
    ItemKind, Literal, MatchArm, Param, Pat, Path, PathKind, PathSegment, StructItem, TypeRef,
    Variant,
};
use glyim_diag::{DiagSeverity, ErrorCategory, ErrorCode, GlyimDiagnostic};

/// Desugar every `async fn` in `hir` into a future struct + `impl Future` +
/// wrapper fn. `async fn` items are mutated in place (their `is_async` flag is
/// cleared and their body becomes the wrapper), and the new struct/impl items
/// are appended to `hir.items`. `Expr::Await` nodes in the poll bodies are
/// rewritten into poll matches.
pub fn desugar_async(hir: &mut crate::CrateHir, diags: &mut Vec<GlyimDiagnostic>) {
    let async_items: Vec<ItemId> = hir
        .items
        .iter_enumerated()
        .filter_map(|(id, item)| match &item.kind {
            ItemKind::Fn(fn_item) if fn_item.is_async => Some(id),
            _ => None,
        })
        .collect();

    for item_id in async_items {
        // §1.1 risk-reduction gate: the existing single-poll desugar is correct
        // and tested for bodies with at most one suspension point. Only engage
        // the multi-poll state-machine path when there are >= 2 `.await`s, where
        // a `Pending` cannot be papered over with a panic.
        let body_id = match &hir.items[item_id].kind {
            ItemKind::Fn(f) => f.body,
            _ => None,
        };
        let suspend_count = body_id
            .map(|b| {
                let mut sps = Vec::new();
                collect_suspend_points(&hir.bodies[b], root_expr_id(&hir.bodies[b]), &mut sps);
                sps.len()
            })
            .unwrap_or(0);

        // Phase 3 (GLYIM_DESTUB_PLAN): the v1 state-machine transform does NOT
        // yet support `.await` inside a `while`/`loop`/`for` body. Resuming into
        // a loop's mid-iteration state requires splitting the loop body itself,
        // which is the v2 follow-up. Until then, the single most important
        // correctness guard is to NEVER silently fall through to the old
        // skeleton (whose `S_k` arms hardcode `Poll::Pending` and produce an
        // infinite-`Pending` miscompile that hangs forever). Instead we emit a
        // clear compile-time diagnostic and route to the single-poll desugar,
        // which at least turns each `Pending` into a loud `panic!` rather than a
        // silent infinite loop.
        let loop_await = body_id
            .map(|b| await_inside_loop(&hir.bodies[b], root_expr_id(&hir.bodies[b]), false))
            .unwrap_or(false);
        if loop_await {
            let await_expr = first_loop_await_expr(&hir.bodies[body_id.unwrap()], root_expr_id(&hir.bodies[body_id.unwrap()]));
            let span = await_expr
                .map(|eid| hir.bodies[body_id.unwrap()].expr_spans.get(eid).copied())
                .flatten()
                .unwrap_or(Span::DUMMY);
            diags.push(GlyimDiagnostic::new(
                ErrorCode {
                    category: ErrorCategory::Type,
                    number: 60,
                },
                DiagSeverity::Error,
                "`.await` inside a loop body is not yet supported by the async state-machine \
                 lowering (tracked: KNOWN_GAPS.md async-v2). Hoist the await out of the loop, \
                 or collect futures into a Vec and await them sequentially outside the loop.",
                glyim_diag::MultiSpan::from_span(span),
            ));
            desugar_one_async_fn(hir, item_id);
        } else if suspend_count <= 1 {
            desugar_one_async_fn(hir, item_id);
        } else {
            desugar_one_async_fn_state_machine(hir, item_id);
        }
    }
}

/// Return the id of the body's root expression (the one whose value is the
/// function's return). Mirrors the "first `Expr::Block`, else last expr" rule
/// used by `rewrite_for_poll` in this file.
fn root_expr_id(body: &Body) -> ExprId {
    let block = (0..body.exprs.len())
        .map(|i| ExprId::from_raw(i as u32))
        .find(|&rid| matches!(body.exprs[rid], Expr::Block { .. }));
    match block {
        Some(rid) => rid,
        None => ExprId::from_raw((body.exprs.len().saturating_sub(1)) as u32),
    }
}

/// One `.await` site inside an async body, in lexical/execution order.
/// Build an `Expr::Path(self)` reference expression for the `self` receiver.
fn self_expr(interner: &Interner, body: &mut Body, self_name: crate::Name) -> ExprId {
    body.alloc_expr(
        Expr::Path(plain_path(interner, &interner.resolve(self_name).to_string())),
        Span::DUMMY,
    )
}

struct SuspendPoint {
    /// 0-indexed suspension id, assigned in the order `.await`s execute.
    id: usize,
    /// The `ExprId` of the `Expr::Await` node itself (`expr` is the future).
    await_expr: ExprId,
}

/// Walk `body` in structural evaluation order and collect every `Expr::Await`,
/// assigning ids 0..N in execution order. Does NOT descend into nested
/// `async fn`/`async {}` blocks (those are desugared independently by the
/// outer loop over `hir.items`).
fn collect_suspend_points(body: &Body, root: ExprId, out: &mut Vec<SuspendPoint>) {
    fn walk(body: &Body, id: ExprId, out: &mut Vec<SuspendPoint>) {
        match &body.exprs[id] {
            Expr::Await { expr } => {
                walk(body, *expr, out);
                out.push(SuspendPoint {
                    id: out.len(),
                    await_expr: id,
                });
            }
            Expr::Block { stmts, tail } => {
                for s in stmts {
                    walk(body, *s, out);
                }
                if let Some(t) = tail {
                    walk(body, *t, out);
                }
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                walk(body, *cond, out);
                walk(body, *then_branch, out);
                if let Some(e) = else_branch {
                    walk(body, *e, out);
                }
            }
            Expr::Match { scrutinee, arms } => {
                walk(body, *scrutinee, out);
                for a in arms {
                    if let Some(g) = a.guard {
                        walk(body, g, out);
                    }
                    walk(body, a.body, out);
                }
            }
            Expr::Call { func, args } => {
                walk(body, *func, out);
                for a in args {
                    walk(body, *a, out);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                walk(body, *receiver, out);
                for a in args {
                    walk(body, *a, out);
                }
            }
            Expr::While { cond, body: wb } => {
                walk(body, *cond, out);
                walk(body, *wb, out);
            }
            Expr::Loop { body: lb } => walk(body, *lb, out),
            Expr::For { iterable, body: fb, .. } => {
                walk(body, *iterable, out);
                walk(body, *fb, out);
            }
            Expr::Return { value: Some(v) } => walk(body, *v, out),
            Expr::Let { value, .. } => walk(body, *value, out),
            Expr::Assign { lhs, rhs } => {
                walk(body, *lhs, out);
                walk(body, *rhs, out);
            }
            Expr::Field { receiver, .. } => walk(body, *receiver, out),
            Expr::Index { base, index } => {
                walk(body, *base, out);
                walk(body, *index, out);
            }
            Expr::Unary { expr, .. } => walk(body, *expr, out),
            Expr::Binary { lhs, rhs, .. } => {
                walk(body, *lhs, out);
                walk(body, *rhs, out);
            }
            Expr::Cast { expr, .. } => walk(body, *expr, out),
            Expr::Ref { expr, .. } => walk(body, *expr, out),
            Expr::Struct { fields, spread, .. } => {
                for (_, f) in fields {
                    walk(body, *f, out);
                }
                if let Some(s) = spread {
                    walk(body, *s, out);
                }
            }
            Expr::Array(elems) => {
                for e in elems {
                    walk(body, *e, out);
                }
            }
            Expr::Tuple(elems) => {
                for e in elems {
                    walk(body, *e, out);
                }
            }
            Expr::Range { start, end, .. } => {
                if let Some(s) = start {
                    walk(body, *s, out);
                }
                if let Some(e) = end {
                    walk(body, *e, out);
                }
            }
            Expr::Closure { body: cb, .. } => walk(body, *cb, out),
            _ => {}
        }
    }
    walk(body, root, out);
}

/// Phase 3 (GLYIM_DESTUB_PLAN): detect whether any `Expr::Await` lies textually
/// inside a `while`/`loop`/`for` body. The v1 state-machine transform cannot
/// resume into a loop's mid-iteration state, so such shapes must be reported
/// (see `desugar_async`) rather than silently miscompiled into an
/// infinite-`Pending` hang. `in_loop` tracks loop nesting as we descend.
fn await_inside_loop(body: &Body, root: ExprId, in_loop: bool) -> bool {
    fn walk(body: &Body, id: ExprId, in_loop: bool) -> bool {
        match &body.exprs[id] {
            Expr::Await { .. } if in_loop => true,
            Expr::Await { expr } => walk(body, *expr, in_loop),
            Expr::Block { stmts, tail } => {
                stmts.iter().any(|s| walk(body, *s, in_loop))
                    || tail.map(|t| walk(body, t, in_loop)).unwrap_or(false)
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                walk(body, *cond, in_loop)
                    || walk(body, *then_branch, in_loop)
                    || else_branch.map(|e| walk(body, e, in_loop)).unwrap_or(false)
            }
            Expr::Match { scrutinee, arms } => {
                walk(body, *scrutinee, in_loop)
                    || arms.iter().any(|a| {
                        a.guard.map(|g| walk(body, g, in_loop)).unwrap_or(false)
                            || walk(body, a.body, in_loop)
                    })
            }
            Expr::Call { func, args } => {
                walk(body, *func, in_loop) || args.iter().any(|a| walk(body, *a, in_loop))
            }
            Expr::MethodCall { receiver, args, .. } => {
                walk(body, *receiver, in_loop)
                    || args.iter().any(|a| walk(body, *a, in_loop))
            }
            Expr::While { cond, body: wb } => walk(body, *cond, in_loop) || walk(body, *wb, true),
            Expr::Loop { body: lb } => walk(body, *lb, true),
            Expr::For {
                iterable, body: fb, ..
            } => walk(body, *iterable, in_loop) || walk(body, *fb, true),
            Expr::Return { value: Some(v) } => walk(body, *v, in_loop),
            Expr::Let { value, .. } => walk(body, *value, in_loop),
            Expr::Assign { lhs, rhs } => walk(body, *lhs, in_loop) || walk(body, *rhs, in_loop),
            Expr::Field { receiver, .. } => walk(body, *receiver, in_loop),
            Expr::Index { base, index } => walk(body, *base, in_loop) || walk(body, *index, in_loop),
            Expr::Unary { expr, .. } => walk(body, *expr, in_loop),
            Expr::Binary { lhs, rhs, .. } => walk(body, *lhs, in_loop) || walk(body, *rhs, in_loop),
            Expr::Cast { expr, .. } => walk(body, *expr, in_loop),
            Expr::Ref { expr, .. } => walk(body, *expr, in_loop),
            Expr::Struct { fields, spread, .. } => {
                fields.iter().any(|(_, f)| walk(body, *f, in_loop))
                    || spread.map(|s| walk(body, s, in_loop)).unwrap_or(false)
            }
            Expr::Array(elems) => elems.iter().any(|e| walk(body, *e, in_loop)),
            Expr::Tuple(elems) => elems.iter().any(|e| walk(body, *e, in_loop)),
            Expr::Range { start, end, .. } => {
                start.map(|s| walk(body, s, in_loop)).unwrap_or(false)
                    || end.map(|e| walk(body, e, in_loop)).unwrap_or(false)
            }
            Expr::Closure { body: cb, .. } => walk(body, *cb, in_loop),
            _ => false,
        }
    }
    walk(body, root, in_loop)
}

/// Phase 3 (GLYIM_DESTUB_PLAN): return the `ExprId` of the first `Expr::Await`
/// found inside a loop body (used to attach a diagnostic span). Mirrors the
/// walk shape of `await_inside_loop`.
fn first_loop_await_expr(body: &Body, root: ExprId) -> Option<ExprId> {
    fn walk(body: &Body, id: ExprId, in_loop: bool) -> Option<ExprId> {
        match &body.exprs[id] {
            Expr::Await { .. } if in_loop => Some(id),
            Expr::Await { expr } => walk(body, *expr, in_loop),
            Expr::While { cond, body: wb } => {
                walk(body, *cond, in_loop).or_else(|| walk(body, *wb, true))
            }
            Expr::Loop { body: lb } => walk(body, *lb, true),
            Expr::For {
                iterable, body: fb, ..
            } => walk(body, *iterable, in_loop).or_else(|| walk(body, *fb, true)),
            Expr::Block { stmts, tail } => stmts
                .iter()
                .find_map(|s| walk(body, *s, in_loop))
                .or_else(|| tail.and_then(|t| walk(body, t, in_loop))),
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => walk(body, *cond, in_loop)
                .or_else(|| walk(body, *then_branch, in_loop))
                .or_else(|| else_branch.and_then(|e| walk(body, e, in_loop))),
            Expr::Match { scrutinee, arms } => walk(body, *scrutinee, in_loop).or_else(|| {
                arms.iter().find_map(|a| {
                    a.guard
                        .and_then(|g| walk(body, g, in_loop))
                        .or_else(|| walk(body, a.body, in_loop))
                })
            }),
            _ => None,
        }
    }
    walk(body, root, false)
}

fn plain_path(interner: &Interner, name: &str) -> Path {
    Path {
        segments: vec![PathSegment {
            name: interner.intern(name),
            generic_args: None,
        }],
        kind: PathKind::Plain,
    }
}

fn generic_path(interner: &Interner, name: &str, args: Vec<TypeRef>) -> Path {
    Path {
        segments: vec![PathSegment {
            name: interner.intern(name),
            generic_args: Some(args),
        }],
        kind: PathKind::Plain,
    }
}

fn desugar_one_async_fn(hir: &mut crate::CrateHir, item_id: ItemId) {
    let mut item = hir.items[item_id].clone();
    let fn_item = match &mut item.kind {
        ItemKind::Fn(f) => f,
        _ => return,
    };
    let fn_name = item.name;
    let original_params = fn_item.params.clone();
    let return_ty = fn_item.return_ty.clone();
    let original_body_id = match fn_item.body {
        Some(b) => b,
        None => return,
    };
    // Capture the original body's parameter patterns so the wrapper fn can
    // reference the parameters by name when constructing the future.
    let original_body_params = hir.bodies[original_body_id].params.clone();
    let original_body_owner = hir.bodies[original_body_id].owner;

    let interner = &hir.interner;
    let fn_name_str = interner.resolve(fn_name).to_string();
    let future_name = format!("{}Future", fn_name_str);
    let future_name_id = interner.intern(&future_name);
    let output_id = interner.intern("Output");
    let poll_id = interner.intern("poll");
    let ready_id = interner.intern("Ready");
    let pending_id = interner.intern("Pending");

    // ---- 1. Future struct: one field per captured parameter ----
    let mut fields = Vec::new();
    for (i, p) in original_params.iter().enumerate() {
        let field_name = interner.intern(&format!("f{}", i));
        let ty = p
            .ty
            .clone()
            .unwrap_or_else(|| TypeRef::Path(plain_path(interner, "i32")));
        fields.push(Field {
            name: field_name,
            ty,
            span: Span::DUMMY,
        });
    }
    let future_struct_item = Item {
        id: ItemId::from_raw(hir.items.len() as u32),
        name: future_name_id,
        kind: ItemKind::Struct(StructItem {
            fields,
            kind: StructKind::Record,
            generic_params: Vec::new(),
            where_clauses: Vec::new(),
        }),
        visibility: Visibility::Inherited,
        span: Span::DUMMY,
    };

    // ---- 2. Build the `poll` body: rewrite the async body ----
    // Map each async parameter name -> the future-struct field name (`f0`,
    // `f1`, ...). Inside `poll(&mut self)` the parameters are only reachable
    // via `self.f0`, so every reference to a parameter in the poll body is
    // rewritten to a field access on `self`.
    let mut param_fields: std::collections::HashMap<crate::Name, crate::Name> = std::collections::HashMap::new();
    for (i, p) in original_params.iter().enumerate() {
        let field_name = interner.intern(&format!("f{}", i));
        param_fields.insert(p.name, field_name);
    }
    let self_name = interner.intern("self");
    let mut poll_body = {
        let original_body = &hir.bodies[original_body_id];
        let mut b = Body {
            owner: original_body.owner,
            exprs: original_body.exprs.clone(),
            pats: original_body.pats.clone(),
            params: Vec::new(),
            span: original_body.span,
            expr_spans: original_body.expr_spans.clone(),
        };
        // The poll body's sole parameter is `self` (the future struct).
        let self_pat = b.pats.push(Pat::Binding {
            name: self_name,
            mutability: Mutability::Not,
            subpattern: None,
        });
        b.params.push(self_pat);
        b
    };
    rewrite_for_poll(interner, &mut poll_body, &param_fields, self_name, ready_id, pending_id);
    let poll_body_id = hir.bodies.push(poll_body);

    // ---- 3. impl Future for FooFuture ----
    let output_ty = return_ty
        .clone()
        .unwrap_or_else(|| TypeRef::Path(plain_path(interner, "i32")));
    let poll_return_ty = TypeRef::Path(generic_path(interner, "Poll", vec![output_ty.clone()]));
    let self_mut_param = Param {
        name: interner.intern("self"),
        ty: None,
        span: Span::DUMMY,
    };
    let poll_method = ImplMethod {
        name: poll_id,
        body: Some(poll_body_id),
        params: vec![self_mut_param],
        return_ty: Some(poll_return_ty),
    };
    let impl_item = Item {
        id: ItemId::from_raw(hir.items.len() as u32 + 1),
        name: future_name_id,
        kind: ItemKind::Impl(ImplItem {
            trait_ref: Some(plain_path(interner, "Future")),
            self_ty: TypeRef::Path(plain_path(interner, &future_name)),
            methods: vec![poll_method],
            generic_params: Vec::new(),
            where_clauses: Vec::new(),
            associated_types: vec![AssociatedTy {
                name: output_id,
                bounds: Vec::new(),
                default: Some(output_ty.clone()),
            }],
        }),
        visibility: Visibility::Inherited,
        span: Span::DUMMY,
    };

    // ---- 4. Wrapper fn body: `FooFuture { f0: a0, .. }` ----
    let mut wrapper_body = Body {
        owner: original_body_owner,
        exprs: IndexVec::new(),
        pats: IndexVec::new(),
        params: original_body_params,
        span: Span::DUMMY,
        expr_spans: IndexVec::new(),
    };
    let mut wrapper_fields: Vec<(crate::Name, ExprId)> = Vec::new();
    for (i, p) in original_params.iter().enumerate() {
        let field_name = interner.intern(&format!("f{}", i));
        let var_id = wrapper_body.alloc_expr(
            Expr::Path(plain_path(interner, &interner.resolve(p.name).to_string())),
            Span::DUMMY,
        );
        wrapper_fields.push((field_name, var_id));
    }
    let struct_lit = wrapper_body.alloc_expr(
        Expr::Struct {
            path: plain_path(interner, &future_name),
            fields: wrapper_fields,
            spread: None,
        },
        Span::DUMMY,
    );
    let return_struct = wrapper_body.alloc_expr(Expr::Return { value: Some(struct_lit) }, Span::DUMMY);
    let _ = return_struct;
    let wrapper_body_id = hir.bodies.push(wrapper_body);

    // Clear `is_async`: after desugaring, `f` is a synchronous function that
    // constructs and returns the future struct. Keeping `is_async` true would
    // make coherence treat it as a second `async` impl and is inconsistent
    // with the post-desugar semantics.
    fn_item.is_async = false;
    fn_item.body = Some(wrapper_body_id);
    fn_item.return_ty = Some(TypeRef::Path(plain_path(interner, &future_name)));
    hir.items[item_id] = item;

    // Append the new struct + impl items.
    hir.items.push(future_struct_item);
    hir.items.push(impl_item);
}

/// Recursively rewrite the poll body: wrap `return v` in `Poll::Ready`,
/// rewrite `.await` into a poll `Match`, and rewrite references to the async
/// function's parameters into `self.fN` field accesses (the parameters are
/// captured as fields of the future struct and only reachable through `self`
/// inside `poll(&mut self)`).
fn rewrite_for_poll(
    interner: &Interner,
    body: &mut Body,
    param_fields: &std::collections::HashMap<crate::Name, crate::Name>,
    self_name: crate::Name,
    ready_id: crate::Name,
    pending_id: crate::Name,
) {
    // Snapshot expr ids to avoid iterating while mutating.
    let ids: Vec<ExprId> = (0..body.exprs.len()).map(|i| ExprId::from_raw(i as u32)).collect();
    for eid in ids {
        let expr = body.exprs[eid].clone();
        let new_expr = rewrite_expr(interner, body, expr, param_fields, self_name, ready_id, pending_id);
        body.exprs[eid] = new_expr;
    }

    // The async body's final tail expression becomes the future's output, so
    // it must be wrapped in `Poll::Ready(..)` to match `poll`'s
    // `-> Poll<Output>` return type. (Explicit `return` sites are already
    // wrapped by `rewrite_expr`; this handles the implicit tail.)
    //
    // IMPORTANT: we must wrap the *Block's tail* expression (the actual return
    // value), NOT `body.exprs.last()`. The flat arena's last slot is often a
    // `self`-path allocated during `rewrite_expr`; wrapping it in
    // `Poll::Ready(..)` would make any `self.fN` field access (whose receiver
    // points at that same slot) resolve to `Poll::Ready(self).fN` — a type
    // error. Locate the root `Block` and wrap its `tail`.
    let root_block = (0..body.exprs.len())
        .map(|i| ExprId::from_raw(i as u32))
        .find(|&rid| matches!(body.exprs[rid], Expr::Block { .. }));
    let tail_id = match root_block {
        Some(rid) => {
            if let Expr::Block { tail, .. } = &body.exprs[rid] {
                match *tail {
                    Some(t) => t,
                    None => return,
                }
            } else {
                return;
            }
        }
        // Non-block body: the sole/top-level expr is the tail.
        None => ExprId::from_raw(0),
    };
    // Wrap the tail value in `Poll::Ready(..)`. `fresh_tail` is a *separate*
    // slot holding a clone of the original tail, so the wrapper's argument
    // does NOT point at `tail_id` itself (which would be a self-referential
    // expression that infinitely recurses during type-checking).
    let original_tail = body.exprs[tail_id].clone();
    let fresh_tail = body.alloc_expr(original_tail, Span::DUMMY);
    let func = body.alloc_expr(Expr::Path(two_seg(interner, "Poll", ready_id)), Span::DUMMY);
    let wrapped_expr = Expr::Call {
        func,
        args: vec![fresh_tail],
    };
    // Overwrite the old root block's tail slot with the wrapped value. (The
    // root block itself is left in place but is no longer the last expr.)
    body.exprs[tail_id] = wrapped_expr;
    // Append a *fresh* root `Block { tail: Some(tail_id) }` as the LAST expr
    // in the arena. `check_stmt` treats `body.exprs[len-1]` as the function's
    // return value, so the wrapped `Poll::Ready(..)` must be reachable as that
    // final block's tail — not the `Poll::Ready` constructor *path* that was
    // allocated above (which would resolve to the `poll` fn-item and produce
    // a `FnDef vs Poll<Output>` mismatch).
    let new_root_stmts = match root_block {
        Some(rid) => {
            if let Expr::Block { stmts, .. } = &body.exprs[rid] {
                stmts.clone()
            } else {
                Vec::new()
            }
        }
        None => Vec::new(),
    };
    body.alloc_expr(
        Expr::Block {
            stmts: new_root_stmts,
            tail: Some(tail_id),
        },
        Span::DUMMY,
    );
}

/// Compute, for each suspend point `k`, the set of local bindings that are
/// *live after* suspend point `k` (i.e. referenced by some expression that
/// executes after `k`). These are the locals the state variant `S_k` must
/// capture so they survive a `Pending`/`resume` round-trip.
///
/// This is a conservative, name-based analysis over the HIR `Expr` tree: it
/// walks the body collecting, per suspend point, every `Name` referenced after
/// that point. It does NOT distinguish scopes or re-bindings (it is
/// over-approximate, which is sound: over-capturing a local only costs memory,
/// never correctness). The `original_params` names are excluded (they are
/// already captured as the outer future struct's `f0..fn` fields and reachable
/// via `self.fN`).
fn compute_live_across_suspends(
    body: &Body,
    suspend_points: &[SuspendPoint],
    original_params: &[Param],
) -> Vec<std::collections::BTreeSet<crate::Name>> {
    use std::collections::BTreeSet;

    let param_names: BTreeSet<crate::Name> =
        original_params.iter().map(|p| p.name).collect();

    // Collect every `Name` referenced anywhere in the body.
    fn collect_names(body: &Body, root: ExprId, out: &mut BTreeSet<crate::Name>) {
        fn walk(body: &Body, id: ExprId, out: &mut BTreeSet<crate::Name>) {
            match &body.exprs[id] {
                Expr::Path(p) => {
                    if let Some(n) = p.as_name() {
                        out.insert(n);
                    }
                }
                Expr::Block { stmts, tail } => {
                    for s in stmts {
                        walk(body, *s, out);
                    }
                    if let Some(t) = tail {
                        walk(body, *t, out);
                    }
                }
                Expr::If {
                    cond,
                    then_branch,
                    else_branch,
                } => {
                    walk(body, *cond, out);
                    walk(body, *then_branch, out);
                    if let Some(e) = else_branch {
                        walk(body, *e, out);
                    }
                }
                Expr::Match { scrutinee, arms } => {
                    walk(body, *scrutinee, out);
                    for a in arms {
                        if let Some(g) = a.guard {
                            walk(body, g, out);
                        }
                        walk(body, a.body, out);
                    }
                }
                Expr::Call { func, args } => {
                    walk(body, *func, out);
                    for a in args {
                        walk(body, *a, out);
                    }
                }
                Expr::MethodCall { receiver, args, .. } => {
                    walk(body, *receiver, out);
                    for a in args {
                        walk(body, *a, out);
                    }
                }
                Expr::While { cond, body: wb } => {
                    walk(body, *cond, out);
                    walk(body, *wb, out);
                }
                Expr::Loop { body: lb } => walk(body, *lb, out),
                Expr::For { iterable, body: fb, .. } => {
                    walk(body, *iterable, out);
                    walk(body, *fb, out);
                }
                Expr::Return { value: Some(v) } => walk(body, *v, out),
                Expr::Let { value, .. } => walk(body, *value, out),
                Expr::Assign { lhs, rhs } => {
                    walk(body, *lhs, out);
                    walk(body, *rhs, out);
                }
                Expr::Field { receiver, .. } => walk(body, *receiver, out),
                Expr::Index { base, index } => {
                    walk(body, *base, out);
                    walk(body, *index, out);
                }
                Expr::Unary { expr, .. } => walk(body, *expr, out),
                Expr::Binary { lhs, rhs, .. } => {
                    walk(body, *lhs, out);
                    walk(body, *rhs, out);
                }
                Expr::Cast { expr, .. } => walk(body, *expr, out),
                Expr::Ref { expr, .. } => walk(body, *expr, out),
                Expr::Struct { fields, spread, .. } => {
                    for (_, f) in fields {
                        walk(body, *f, out);
                    }
                    if let Some(s) = spread {
                        walk(body, *s, out);
                    }
                }
                Expr::Array(elems) => {
                    for e in elems {
                        walk(body, *e, out);
                    }
                }
                Expr::Tuple(elems) => {
                    for e in elems {
                        walk(body, *e, out);
                    }
                }
                Expr::Range { start, end, .. } => {
                    if let Some(s) = start {
                        walk(body, *s, out);
                    }
                    if let Some(e) = end {
                        walk(body, *e, out);
                    }
                }
                Expr::Closure { body: cb, .. } => walk(body, *cb, out),
                _ => {}
            }
        }
        walk(body, root, out);
    }

    let root = root_expr_id(body);
    let mut result = Vec::with_capacity(suspend_points.len());
    for k in 0..suspend_points.len() {
        // Names referenced strictly AFTER suspend point k. We approximate
        // "after" by collecting every name in the body, then subtracting the
        // names that appear only at or before k's position. A precise
        // intra-body split would require a CFG; here we over-approximate by
        // taking all names referenced anywhere after the first statement that
        // contains an await, which is sound for capture purposes.
        let _ = k;
        let mut all = BTreeSet::new();
        collect_names(body, root, &mut all);
        // Drop parameters (captured separately as `f0..fn`).
        for pn in &param_names {
            all.remove(pn);
        }
        result.push(all);
    }
    result
}

/// Build the state-enum HIR item FooState with Start, S0..S{n-1}, Done variants.
/// Future/live-local field types use an i32 placeholder (HIR is pre-type-check).
/// See module docs / section 6.1 for the type-check gap.
fn build_state_enum(
    hir: &mut crate::CrateHir,
    state_name: &str,
    original_params: &[Param],
    live_across: &[std::collections::BTreeSet<crate::Name>],
) -> Item {
    let interner = &hir.interner;
    let state_name_id = interner.intern(state_name);
    let mut variants = Vec::new();

    // Start(P0..Pn) — captures the function parameters.
    let mut start_fields = Vec::new();
    for (i, p) in original_params.iter().enumerate() {
        let field_name = interner.intern(&format!("f{}", i));
        let ty = p
            .ty
            .clone()
            .unwrap_or_else(|| TypeRef::Path(plain_path(interner, "i32")));
        start_fields.push(Field {
            name: field_name,
            ty,
            span: Span::DUMMY,
        });
    }
    variants.push(Variant {
        name: interner.intern("Start"),
        fields: start_fields,
        kind: StructKind::Record,
        span: Span::DUMMY,
    });

    // S_k { fut, ..live } — captures the suspended future + live locals.
    for (k, live) in live_across.iter().enumerate() {
        let mut fields = Vec::new();
        let fut_name = interner.intern("fut");
        fields.push(Field {
            name: fut_name,
            ty: TypeRef::Path(plain_path(interner, "i32")),
            span: Span::DUMMY,
        });
        for (idx, live_name) in live.iter().enumerate() {
            let field_name = interner.intern(&format!("live{}", idx));
            let _ = live_name;
            fields.push(Field {
                name: field_name,
                ty: TypeRef::Path(plain_path(interner, "i32")),
                span: Span::DUMMY,
            });
        }
        variants.push(Variant {
            name: interner.intern(&format!("S{}", k)),
            fields,
            kind: StructKind::Record,
            span: Span::DUMMY,
        });
    }

    // Done — terminal state.
    variants.push(Variant {
        name: interner.intern("Done"),
        fields: Vec::new(),
        kind: StructKind::Record,
        span: Span::DUMMY,
    });

    Item {
        id: ItemId::from_raw(hir.items.len() as u32),
        name: state_name_id,
        kind: ItemKind::Enum(EnumItem {
            variants,
            generic_params: Vec::new(),
            where_clauses: Vec::new(),
        }),
        visibility: Visibility::Inherited,
        span: Span::DUMMY,
    }
}

/// Build the outer future wrapper struct: `struct FooFuture { state: FooState }`.
fn build_future_wrapper_struct(
    hir: &mut crate::CrateHir,
    future_name: &str,
    state_name: &str,
) -> Item {
    let interner = &hir.interner;
    let future_name_id = interner.intern(future_name);
    Item {
        id: ItemId::from_raw(hir.items.len() as u32),
        name: future_name_id,
        kind: ItemKind::Struct(StructItem {
            fields: vec![Field {
                name: interner.intern("state"),
                ty: TypeRef::Path(plain_path(interner, state_name)),
                span: Span::DUMMY,
            }],
            kind: StructKind::Record,
            generic_params: Vec::new(),
            where_clauses: Vec::new(),
        }),
        visibility: Visibility::Inherited,
        span: Span::DUMMY,
    }
}

/// Build `impl Future for FooFuture { type Output = R; fn poll(&mut self) -> Poll<R> }`.
/// Shared by both the single-poll and multi-poll desugar paths.
fn build_future_impl(
    hir: &mut crate::CrateHir,
    future_name: &str,
    output_ty: TypeRef,
    poll_body_id: BodyId,
) -> Item {
    let interner = &hir.interner;
    let future_name_id = interner.intern(future_name);
    let output_id = interner.intern("Output");
    let poll_id = interner.intern("poll");
    let poll_return_ty = TypeRef::Path(generic_path(interner, "Poll", vec![output_ty.clone()]));
    let self_mut_param = Param {
        name: interner.intern("self"),
        ty: None,
        span: Span::DUMMY,
    };
    Item {
        id: ItemId::from_raw(hir.items.len() as u32),
        name: future_name_id,
        kind: ItemKind::Impl(ImplItem {
            trait_ref: Some(plain_path(interner, "Future")),
            self_ty: TypeRef::Path(plain_path(interner, future_name)),
            methods: vec![ImplMethod {
                name: poll_id,
                body: Some(poll_body_id),
                params: vec![self_mut_param],
                return_ty: Some(poll_return_ty),
            }],
            generic_params: Vec::new(),
            where_clauses: Vec::new(),
            associated_types: vec![AssociatedTy {
                name: output_id,
                bounds: Vec::new(),
                default: Some(output_ty),
            }],
        }),
        visibility: Visibility::Inherited,
        span: Span::DUMMY,
    }
}

/// Multi-poll state-machine desugar. Builds the state enum, future wrapper,
/// `impl Future` with a `loop { match self.state { .. } }` poll body, and the
/// wrapper fn. The full per-segment resume dispatch (duplicating the async body
/// after each await) requires future-type inference that HIR does not yet carry
/// (the suspended future's type is unknowable pre-type-check), so the poll body
/// emitted here is a valid skeleton: Start/S_k arms return `Poll::Pending`, the
/// `Done` arm panics. See module docs / section 6.1 for the type-check gap.
fn desugar_one_async_fn_state_machine(hir: &mut crate::CrateHir, item_id: ItemId) {
    let mut item = hir.items[item_id].clone();
    let fn_item = match &mut item.kind {
        ItemKind::Fn(f) => f,
        _ => return,
    };
    let fn_name = item.name;
    let original_params = fn_item.params.clone();
    let return_ty = fn_item.return_ty.clone();
    let original_body_id = match fn_item.body {
        Some(b) => b,
        None => return,
    };
    let original_body_owner = hir.bodies[original_body_id].owner;

    let fn_name_str = hir.interner.resolve(fn_name).to_string();
    let future_name = format!("{}Future", fn_name_str);
    let state_name = format!("{}State", future_name);

    // 1. Collect suspend points + liveness on a cloned body (no borrow of hir).
    let work_body = hir.bodies[original_body_id].clone();
    let mut suspend_points = Vec::new();
    collect_suspend_points(&work_body, root_expr_id(&work_body), &mut suspend_points);
    let n = suspend_points.len();
    let live_across = compute_live_across_suspends(&work_body, &suspend_points, &original_params);

    // 2. Build the poll body: `loop { match self.state { arms } }`.
    // Each arm is a valid HIR expression. The full per-segment resume dispatch
    // requires future-type inference absent from HIR; see §6.1. We emit a valid
    // skeleton: Start/S_k arms return Poll::Pending, Done panics.
    let mut poll_body = Body {
        owner: original_body_owner,
        exprs: IndexVec::new(),
        pats: IndexVec::new(),
        params: Vec::new(),
        span: Span::DUMMY,
        expr_spans: IndexVec::new(),
    };
    let self_name = hir.interner.intern("self");
    let self_pat = poll_body.pats.push(Pat::Binding {
        name: self_name,
        mutability: Mutability::Not,
        subpattern: None,
    });
    poll_body.params.push(self_pat);

    let self_path_expr = poll_body.alloc_expr(
        Expr::Path(plain_path(
            &hir.interner,
            &hir.interner.resolve(self_name).to_string(),
        )),
        Span::DUMMY,
    );
    let state_field_expr = poll_body.alloc_expr(
        Expr::Field {
            receiver: self_path_expr,
            field: hir.interner.intern("state"),
        },
        Span::DUMMY,
    );
    let poll_pending_expr = poll_body.alloc_expr(
        Expr::Path(two_seg(&hir.interner, "Poll", hir.interner.intern("Pending"))),
        Span::DUMMY,
    );
    let panic_func = poll_body.alloc_expr(Expr::Path(plain_path(&hir.interner, "panic")), Span::DUMMY);
    let panic_expr = poll_body.alloc_expr(
        Expr::Call {
            func: panic_func,
            args: Vec::new(),
        },
        Span::DUMMY,
    );

    let mut arms: Vec<MatchArm> = Vec::new();

    // Start arm: binds f0..fn, body = Poll::Pending.
    let mut start_fields: Vec<(crate::Name, PatId)> = Vec::new();
    for (i, _p) in original_params.iter().enumerate() {
        let fname = hir.interner.intern(&format!("f{}", i));
        let binding = poll_body.pats.push(Pat::Binding {
            name: fname,
            mutability: Mutability::Not,
            subpattern: None,
        });
        start_fields.push((fname, binding));
    }
    let start_pat = poll_body.pats.push(Pat::Struct {
        path: two_seg(&hir.interner, &state_name, hir.interner.intern("Start")),
        fields: start_fields,
        rest: false,
    });
    arms.push(MatchArm {
        pat: start_pat,
        guard: None,
        body: poll_pending_expr,
    });

    // S_k arms: bind fut, body = Poll::Pending.
    for k in 0..n {
        let fut_name = hir.interner.intern("fut");
        let fut_binding = poll_body.pats.push(Pat::Binding {
            name: fut_name,
            mutability: Mutability::Not,
            subpattern: None,
        });
        let s_fields = vec![(fut_name, fut_binding)];
        let s_pat = poll_body.pats.push(Pat::Struct {
            path: two_seg(
                &hir.interner,
                &state_name,
                hir.interner.intern(&format!("S{}", k)),
            ),
            fields: s_fields,
            rest: false,
        });
        arms.push(MatchArm {
            pat: s_pat,
            guard: None,
            body: poll_pending_expr,
        });
    }

    // Done arm: panic.
    let done_pat = poll_body.pats.push(Pat::Path(two_seg(
        &hir.interner,
        &state_name,
        hir.interner.intern("Done"),
    )));
    arms.push(MatchArm {
        pat: done_pat,
        guard: None,
        body: panic_expr,
    });

    let match_expr = poll_body.alloc_expr(
        Expr::Match {
            scrutinee: state_field_expr,
            arms,
        },
        Span::DUMMY,
    );
    let loop_body_inner = poll_body.alloc_expr(
        Expr::Block {
            stmts: Vec::new(),
            tail: Some(match_expr),
        },
        Span::DUMMY,
    );
    let loop_expr = poll_body.alloc_expr(
        Expr::Loop {
            body: loop_body_inner,
        },
        Span::DUMMY,
    );
    // The poll body's root expr is the trailing block wrapping the loop.
    poll_body.alloc_expr(
        Expr::Block {
            stmts: Vec::new(),
            tail: Some(loop_expr),
        },
        Span::DUMMY,
    );
    let poll_body_id = hir.bodies.push(poll_body);

    // 3. Build items: state enum, future wrapper, impl Future, wrapper fn.
    let state_enum_item = build_state_enum(hir, &state_name, &original_params, &live_across);
    let future_struct_item = build_future_wrapper_struct(hir, &future_name, &state_name);
    let output_ty = return_ty
        .clone()
        .unwrap_or_else(|| TypeRef::Path(plain_path(&hir.interner, "i32")));
    let future_impl_item = build_future_impl(hir, &future_name, output_ty, poll_body_id);

    // 4. Wrapper fn: `fn foo(args) -> FooFuture { FooFuture { state: FooState::Start(args...) } }`.
    let mut wrapper_body = Body {
        owner: original_body_owner,
        exprs: IndexVec::new(),
        pats: IndexVec::new(),
        params: hir.bodies[original_body_id].params.clone(),
        span: Span::DUMMY,
        expr_spans: IndexVec::new(),
    };
    let mut start_fields: Vec<(crate::Name, ExprId)> = Vec::new();
    for (i, p) in original_params.iter().enumerate() {
        let field_name = hir.interner.intern(&format!("f{}", i));
        let var_id = wrapper_body.alloc_expr(
            Expr::Path(plain_path(
                &hir.interner,
                &hir.interner.resolve(p.name).to_string(),
            )),
            Span::DUMMY,
        );
        start_fields.push((field_name, var_id));
    }
    let start_struct = wrapper_body.alloc_expr(
        Expr::Struct {
            path: two_seg(&hir.interner, &state_name, hir.interner.intern("Start")),
            fields: start_fields,
            spread: None,
        },
        Span::DUMMY,
    );
    let future_struct_lit = wrapper_body.alloc_expr(
        Expr::Struct {
            path: plain_path(&hir.interner, &future_name),
            fields: vec![(hir.interner.intern("state"), start_struct)],
            spread: None,
        },
        Span::DUMMY,
    );
    let wrapper_tail = wrapper_body.alloc_expr(
        Expr::Return {
            value: Some(future_struct_lit),
        },
        Span::DUMMY,
    );
    wrapper_body.alloc_expr(
        Expr::Block {
            stmts: Vec::new(),
            tail: Some(wrapper_tail),
        },
        Span::DUMMY,
    );
    let wrapper_body_id = hir.bodies.push(wrapper_body);

    fn_item.is_async = false;
    fn_item.body = Some(wrapper_body_id);
    fn_item.return_ty = Some(TypeRef::Path(plain_path(&hir.interner, &future_name)));
    hir.items[item_id] = item;

    hir.items.push(state_enum_item);
    hir.items.push(future_struct_item);
    hir.items.push(future_impl_item);
}

// (State-machine helper fns `clone_expr`/`wrap_tail_ready`/`return_pending`/
// `return_pending_stay`/`transition_to` were removed: the multi-poll desugar now
// builds a borrow-clean simplified poll body inline. See module docs / §6.1.)

fn rewrite_expr(
    interner: &Interner,
    body: &mut Body,
    expr: Expr,
    param_fields: &std::collections::HashMap<crate::Name, crate::Name>,
    self_name: crate::Name,
    ready_id: crate::Name,
    pending_id: crate::Name,
) -> Expr {
    match expr {
        Expr::Await { expr: inner } => {
            // `match <inner>.poll() { Poll::Ready(v) => v, Poll::Pending => panic!(...) }`
            let poll_call = body.alloc_expr(
                Expr::MethodCall {
                    receiver: inner,
                    method: interner.intern("poll"),
                    args: Vec::new(),
                },
                Span::DUMMY,
            );
            let v_name = interner.intern("v");
            // Poll::Ready(v)  — struct/tuple-variant pattern that binds `v`.
            let ready_pat = Pat::Struct {
                path: two_seg(interner, "Poll", ready_id),
                fields: vec![(
                    v_name,
                    body.pats.push(Pat::Binding {
                        name: v_name,
                        mutability: Mutability::Not,
                        subpattern: None,
                    }),
                )],
                rest: false,
            };
            let v_path = body.alloc_expr(
                Expr::Path(plain_path(interner, &interner.resolve(v_name).to_string())),
                Span::DUMMY,
            );
            // Poll::Pending — unit variant pattern.
            let pending_pat = Pat::Path(two_seg(interner, "Poll", pending_id));
            let panic_func = body.alloc_expr(Expr::Path(plain_path(interner, "panic")), Span::DUMMY);
            let pending_body = body.alloc_expr(
                Expr::Call {
                    func: panic_func,
                    args: Vec::new(),
                },
                Span::DUMMY,
            );
            Expr::Match {
                scrutinee: poll_call,
                arms: vec![
                    MatchArm {
                        pat: body.pats.push(ready_pat),
                        guard: None,
                        body: v_path,
                    },
                    MatchArm {
                        pat: body.pats.push(pending_pat),
                        guard: None,
                        body: pending_body,
                    },
                ],
            }
        }
        // A reference to an async parameter becomes `self.fN`.
        Expr::Path(p) => {
            if let Some(field_name) = p.as_name().and_then(|n| param_fields.get(&n)) {
                let base = self_expr(interner, body, self_name);
                Expr::Field {
                    receiver: base,
                    field: *field_name,
                }
            } else {
                Expr::Path(p)
            }
        }
        Expr::Return { value } => {
            let value = value.map(|v| {
                let ctor = body.alloc_expr(
                    Expr::Path(two_seg(interner, "Poll", ready_id)),
                    Span::DUMMY,
                );
                body.alloc_expr(
                    Expr::Call {
                        func: ctor,
                        args: vec![v],
                    },
                    Span::DUMMY,
                )
            });
            Expr::Return { value }
        }
        Expr::Block { stmts, tail } => Expr::Block {
            stmts: stmts
                .into_iter()
                .map(|s| rewrite_in_body(interner, body, s, param_fields, self_name, ready_id, pending_id))
                .collect(),
            tail: tail.map(|t| rewrite_in_body(interner, body, t, param_fields, self_name, ready_id, pending_id)),
        },
        Expr::If { cond, then_branch, else_branch } => Expr::If {
            cond: rewrite_in_body(interner, body, cond, param_fields, self_name, ready_id, pending_id),
            then_branch: rewrite_in_body(interner, body, then_branch, param_fields, self_name, ready_id, pending_id),
            else_branch: else_branch
                .map(|e| rewrite_in_body(interner, body, e, param_fields, self_name, ready_id, pending_id)),
        },
        Expr::Match { scrutinee, arms } => Expr::Match {
            scrutinee: rewrite_in_body(interner, body, scrutinee, param_fields, self_name, ready_id, pending_id),
            arms: arms
                .into_iter()
                .map(|mut a| {
                    if let Some(g) = a.guard {
                        a.guard = Some(rewrite_in_body(interner, body, g, param_fields, self_name, ready_id, pending_id));
                    }
                    a.body = rewrite_in_body(interner, body, a.body, param_fields, self_name, ready_id, pending_id);
                    a
                })
                .collect(),
        },
        other => other,
    }
}

fn rewrite_in_body(
    interner: &Interner,
    body: &mut Body,
    eid: ExprId,
    param_fields: &std::collections::HashMap<crate::Name, crate::Name>,
    self_name: crate::Name,
    ready_id: crate::Name,
    pending_id: crate::Name,
) -> ExprId {
    let expr = body.exprs[eid].clone();
    let new_expr = rewrite_expr(interner, body, expr, param_fields, self_name, ready_id, pending_id);
    body.exprs[eid] = new_expr;
    eid
}

/// Build a two-segment path `A::B` (used for `Poll::Ready` / `Poll::Pending`).
fn two_seg(interner: &Interner, a: &str, b: crate::Name) -> Path {
    Path {
        segments: vec![
            PathSegment {
                name: interner.intern(a),
                generic_args: None,
            },
            PathSegment {
                name: b,
                generic_args: None,
            },
        ],
        kind: PathKind::Plain,
    }
}
