//! Body type-checking: HIR → THIR with inference.

use glyim_core::def_id::{CrateId, LocalDefId};
use glyim_core::interner::Name;
use glyim_core::primitives::*;
use glyim_diag::GlyimDiagnostic;
use glyim_hir::*;
use glyim_solve::{InferenceTable, Obligation};
use glyim_span::Span;
use glyim_type::*;

use crate::thir::{self, LocalVarId};

pub(crate) fn check_function_body(
    ctx: &mut TyCtxMut,
    infer: &mut InferenceTable,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    pending_obligations: &mut Vec<Obligation>,
    body_id: BodyId,
    hir: &CrateHir,
    local_def_id: LocalDefId,
    return_ty: Ty,
    params: &[glyim_hir::Param],
) -> thir::Body {
    let body = &hir.bodies[body_id];
    let mut local_var_map = std::collections::HashMap::new();
    let mut thir_params = Vec::new();

    // Process parameters
    for (i, param) in params.iter().enumerate() {
        let ty = match &param.ty {
            Some(_) => {
                let var = infer.new_ty_var(ctx);
                ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
            }
            None => {
                let var = infer.new_ty_var(ctx);
                ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
            }
        };
        let local_id = LocalVarId::from_raw(i as u32);
        local_var_map.insert(param.name, local_id);
        thir_params.push(thir::Param {
            name: param.name,
            ty,
            span: param.span,
            pat: thir::Pattern {
                kind: thir::PatternKind::Binding {
                    name: param.name,
                    mutability: Mutability::Not,
                    subpattern: None,
                },
                ty,
                span: param.span,
            },
        });
    }

    // Process all body expressions: non-final become statements, final becomes tail
    let mut thir_stmts = Vec::new();
    let mut tail_expr: Option<thir::Expr> = None;
    let len = body.exprs.len();
    for (idx, (expr_id, _expr)) in body.exprs.iter_enumerated().enumerate() {
        let (thir_expr, expr_ty) = check_expr(
            ctx,
            infer,
            diagnostics,
            pending_obligations,
            hir,
            body,
            &local_var_map,
            expr_id,
        );
        if idx == len - 1 {
            // Tail expression: unify with return type
            let span = body.span;
            if let Err(diags) = infer.unify(ctx, expr_ty, return_ty, span) {
                diagnostics.extend(diags);
            }
            tail_expr = Some(thir_expr);
        } else {
            // Non-tail: push as statement expression
            thir_stmts.push(crate::thir::Stmt::Expr { expr: thir_expr });
        }
    }
    let _return_expr = tail_expr;

    thir::Body {
        owner: glyim_core::def_id::DefId::new(CrateId::from_raw(0), local_def_id),
        params: thir_params,
        return_ty,
        stmts: thir_stmts,
        span: body.span,
    }
}

/// Check for obvious type mismatches before unification.
/// Returns true if the types can possibly be unified (any type is infer/error or they match).
fn types_compatible(ctx: &TyCtxMut, a: Ty, b: Ty) -> bool {
    if a == Ty::ERROR || b == Ty::ERROR || a == Ty::NEVER || b == Ty::NEVER {
        return true;
    }
    let kind_a = ctx.ty_kind(a);
    let kind_b = ctx.ty_kind(b);
    match (kind_a, kind_b) {
        (TyKind::Infer(_), _) | (_, TyKind::Infer(_)) => true,
        // Compare simple variants
        _ => std::mem::discriminant(kind_a) == std::mem::discriminant(kind_b),
    }
}

fn check_expr(
    ctx: &mut TyCtxMut,
    infer: &mut InferenceTable,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    pending_obligations: &mut Vec<Obligation>,
    hir: &CrateHir,
    body: &Body,
    local_var_map: &std::collections::HashMap<Name, LocalVarId>,
    expr_id: ExprId,
) -> (thir::Expr, Ty) {
    let expr = &body.exprs[expr_id];
    match expr {
        Expr::Literal(lit) => {
            let ty = literal_ty(ctx, lit);
            let span = Span::DUMMY; // TODO: proper span from body
            let thir_expr = thir::Expr {
                kind: thir::ExprKind::Literal(thir_literal(lit)),
                ty,
                span,
            };
            (thir_expr, ty)
        }
        Expr::Path(path) => {
            // Look up local variable
            if let Some(name) = path.as_name() {
                if let Some(&local_id) = local_var_map.get(&name) {
                    // For now, return the parameter's type (which is an inference var)
                    let idx = local_id.to_raw() as usize;
                    let param_ty = if idx < body.params.len() {
                        // We need the THIR param type; we don't have it yet. Temporary hack: create a new var.
                        {
                            let var = infer.new_ty_var(ctx);
                            ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
                        }
                    } else {
                        ctx.mk_ty(TyKind::Error)
                    };
                    let thir_expr = thir::Expr {
                        kind: thir::ExprKind::VarRef(local_id),
                        ty: param_ty,
                        span: Span::DUMMY,
                    };
                    return (thir_expr, param_ty);
                }
            }
            diagnostics.push(GlyimDiagnostic::type_error(Span::DUMMY, "unresolved name"));
            (
                thir::Expr {
                    kind: thir::ExprKind::Err,
                    ty: Ty::ERROR,
                    span: Span::DUMMY,
                },
                Ty::ERROR,
            )
        }
        Expr::Binary { op, lhs, rhs } => {
            let (lhs_expr, lhs_ty) = check_expr(
                ctx,
                infer,
                diagnostics,
                pending_obligations,
                hir,
                body,
                local_var_map,
                *lhs,
            );
            let (rhs_expr, rhs_ty) = check_expr(
                ctx,
                infer,
                diagnostics,
                pending_obligations,
                hir,
                body,
                local_var_map,
                *rhs,
            );
            // Quick mismatch check
            if !types_compatible(ctx, lhs_ty, rhs_ty) {
                diagnostics.push(GlyimDiagnostic::type_error(Span::DUMMY, "mismatched types"));
                (
                    thir::Expr {
                        kind: thir::ExprKind::Err,
                        ty: Ty::ERROR,
                        span: Span::DUMMY,
                    },
                    Ty::ERROR,
                )
            } else if let Err(diags) = infer.unify(ctx, lhs_ty, rhs_ty, Span::DUMMY) {
                diagnostics.extend(diags);
                (
                    thir::Expr {
                        kind: thir::ExprKind::Err,
                        ty: Ty::ERROR,
                        span: Span::DUMMY,
                    },
                    Ty::ERROR,
                )
            } else {
                let result_ty = lhs_ty;
                (
                    thir::Expr {
                        kind: thir::ExprKind::Binary {
                            op: *op,
                            lhs: Box::new(lhs_expr),
                            rhs: Box::new(rhs_expr),
                        },
                        ty: result_ty,
                        span: Span::DUMMY,
                    },
                    result_ty,
                )
            }
        }
        Expr::Ref { expr, mutability } => {
            let (inner_expr, inner_ty) = check_expr(
                ctx,
                infer,
                diagnostics,
                pending_obligations,
                hir,
                body,
                local_var_map,
                *expr,
            );
            let ref_ty = ctx.mk_ref(Region::Erased, inner_ty, *mutability);
            (
                thir::Expr {
                    kind: thir::ExprKind::Ref {
                        mutability: *mutability,
                        operand: Box::new(inner_expr),
                    },
                    ty: ref_ty,
                    span: Span::DUMMY,
                },
                ref_ty,
            )
        }
        _ => {
            diagnostics.push(GlyimDiagnostic::type_error(
                Span::DUMMY,
                format!("unsupported expression: {:?}", expr),
            ));
            (
                thir::Expr {
                    kind: thir::ExprKind::Err,
                    ty: Ty::ERROR,
                    span: Span::DUMMY,
                },
                Ty::ERROR,
            )
        }
    }
}

fn literal_ty(ctx: &mut TyCtxMut, lit: &Literal) -> Ty {
    match lit {
        Literal::Int(_, Some(IntTy::I32)) | Literal::Int(_, None) => {
            ctx.mk_ty(TyKind::Int(IntTy::I32))
        }
        Literal::Bool(_) => Ty::BOOL,
        Literal::Unit => Ty::UNIT,
        _ => Ty::ERROR,
    }
}

fn thir_literal(lit: &Literal) -> thir::Literal {
    match lit {
        Literal::Int(val, hint) => thir::Literal::Int(*val, *hint),
        Literal::Uint(val, hint) => thir::Literal::Uint(*val, *hint),
        Literal::Float(bits, ft) => thir::Literal::FloatBits(*bits, *ft),
        Literal::Bool(b) => thir::Literal::Bool(*b),
        Literal::Char(c) => thir::Literal::Char(*c),
        Literal::String(name) => thir::Literal::String(*name),
        Literal::Unit => thir::Literal::Unit,
    }
}
