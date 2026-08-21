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
use glyim_span::Span;

use crate::{
    AssociatedTy, Body, Expr, ExprId, Field, ImplItem, ImplMethod, Item, ItemId, ItemKind, MatchArm,
    Param, Pat, Path, PathKind, PathSegment, StructItem, TypeRef,
};

/// Desugar every `async fn` in `hir` into a future struct + `impl Future` +
/// wrapper fn. `async fn` items are mutated in place (their `is_async` flag is
/// cleared and their body becomes the wrapper), and the new struct/impl items
/// are appended to `hir.items`. `Expr::Await` nodes in the poll bodies are
/// rewritten into poll matches.
pub fn desugar_async(hir: &mut crate::CrateHir) {
    let async_items: Vec<ItemId> = hir
        .items
        .iter_enumerated()
        .filter_map(|(id, item)| match &item.kind {
            ItemKind::Fn(fn_item) if fn_item.is_async => Some(id),
            _ => None,
        })
        .collect();

    for item_id in async_items {
        desugar_one_async_fn(hir, item_id);
    }
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

/// Build an `Expr::Path(self)` reference expression for the `self` receiver.
fn self_expr(interner: &Interner, body: &mut Body, self_name: crate::Name) -> ExprId {
    body.alloc_expr(Expr::Path(plain_path(interner, &interner.resolve(self_name).to_string())), Span::DUMMY)
}

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
