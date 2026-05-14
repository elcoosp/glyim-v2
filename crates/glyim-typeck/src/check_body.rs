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

/// Context struct to bundle the many mutable references needed during expression checking.
#[allow(dead_code)]
struct CheckCtx<'a, 'b, 'c, 'd, 'e> {
    ctx: &'a mut TyCtxMut,
    infer: &'b mut InferenceTable,
    diagnostics: &'c mut Vec<GlyimDiagnostic>,
    pending_obligations: &'d mut Vec<Obligation>,
    hir: &'e CrateHir,
}

#[allow(clippy::too_many_arguments)]
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
        let ty = {
            let var = infer.new_ty_var(ctx);
            ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
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

    let mut chk = CheckCtx {
        ctx,
        infer,
        diagnostics,
        pending_obligations,
        hir,
    };

    // Process all body expressions: non-final become statements, final becomes tail
    let mut thir_stmts = Vec::new();
    let mut _tail_expr: Option<thir::Expr> = None;
    let len = body.exprs.len();
    for (pos, (expr_id, _expr)) in body.exprs.iter_enumerated().enumerate() {
        let (thir_expr, expr_ty) = check_expr(&mut chk, body, &local_var_map, expr_id);
        if pos == len - 1 {
            // Tail expression: unify with return type
            let span = body.span;
            if let Err(diags) = chk.infer.unify(chk.ctx, expr_ty, return_ty, span) {
                chk.diagnostics.extend(diags);
            }
            _tail_expr = Some(thir_expr);
        } else {
            // Non-tail: push as statement expression
            thir_stmts.push(crate::thir::Stmt::Expr { expr: thir_expr });
        }
    }

    thir::Body {
        owner: glyim_core::def_id::DefId::new(CrateId::from_raw(0), local_def_id),
        params: thir_params,
        return_ty,
        stmts: thir_stmts,
        span: body.span,
    }
}

/// Check for obvious type mismatches before unification.
fn types_compatible(ctx: &TyCtxMut, a: Ty, b: Ty) -> bool {
    if a == Ty::ERROR || b == Ty::ERROR || a == Ty::NEVER || b == Ty::NEVER {
        return true;
    }
    let kind_a = ctx.ty_kind(a);
    let kind_b = ctx.ty_kind(b);
    match (kind_a, kind_b) {
        (TyKind::Infer(_), _) | (_, TyKind::Infer(_)) => true,
        _ => std::mem::discriminant(kind_a) == std::mem::discriminant(kind_b),
    }
}

fn check_expr(
    chk: &mut CheckCtx,
    body: &Body,
    local_var_map: &std::collections::HashMap<Name, LocalVarId>,
    expr_id: ExprId,
) -> (thir::Expr, Ty) {
    let expr = &body.exprs[expr_id];
    match expr {
        Expr::Literal(lit) => {
            let ty = literal_ty(chk.ctx, lit);
            let span = Span::DUMMY;
            let thir_expr = thir::Expr {
                kind: thir::ExprKind::Literal(thir_literal(lit)),
                ty,
                span,
            };
            (thir_expr, ty)
        }
        Expr::Path(path) => {
            if let Some(name) = path.as_name()
                && let Some(&local_id) = local_var_map.get(&name)
            {
                let idx = local_id.to_raw() as usize;
                let param_ty = if idx < body.params.len() {
                    let var = chk.infer.new_ty_var(chk.ctx);
                    chk.ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
                } else {
                    chk.ctx.mk_ty(TyKind::Error)
                };
                let thir_expr = thir::Expr {
                    kind: thir::ExprKind::VarRef(local_id),
                    ty: param_ty,
                    span: Span::DUMMY,
                };
                return (thir_expr, param_ty);
            }
            chk.diagnostics
                .push(GlyimDiagnostic::type_error(Span::DUMMY, "unresolved name"));
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
            let (lhs_expr, lhs_ty) = check_expr(chk, body, local_var_map, *lhs);
            let (rhs_expr, rhs_ty) = check_expr(chk, body, local_var_map, *rhs);
            if !types_compatible(chk.ctx, lhs_ty, rhs_ty) {
                chk.diagnostics
                    .push(GlyimDiagnostic::type_error(Span::DUMMY, "mismatched types"));
                (
                    thir::Expr {
                        kind: thir::ExprKind::Err,
                        ty: Ty::ERROR,
                        span: Span::DUMMY,
                    },
                    Ty::ERROR,
                )
            } else if let Err(diags) = chk.infer.unify(chk.ctx, lhs_ty, rhs_ty, Span::DUMMY) {
                chk.diagnostics.extend(diags);
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
            let (inner_expr, inner_ty) = check_expr(chk, body, local_var_map, *expr);
            let ref_ty = chk.ctx.mk_ref(Region::Erased, inner_ty, *mutability);
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
        Expr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let (cond_expr, cond_ty) = check_expr(chk, body, local_var_map, *cond);
            // Condition must be bool
            if let Err(diags) = chk.infer.unify(chk.ctx, cond_ty, Ty::BOOL, Span::DUMMY) {
                chk.diagnostics.extend(diags);
            }
            let (then_expr, then_ty) = check_expr(chk, body, local_var_map, *then_branch);
            if let Some(else_id) = else_branch {
                let (_else_expr, else_ty) = check_expr(chk, body, local_var_map, *else_id);
                // Unify then and else branches
                if let Err(diags) = chk.infer.unify(chk.ctx, then_ty, else_ty, Span::DUMMY) {
                    chk.diagnostics.extend(diags);
                }
            }
            let result_ty = if then_ty != Ty::ERROR {
                then_ty
            } else {
                Ty::UNIT
            };
            (
                thir::Expr {
                    kind: thir::ExprKind::If {
                        cond: Box::new(cond_expr),
                        then_branch: Box::new(then_expr),
                        else_branch: else_branch.map(|_| {
                            Box::new(thir::Expr {
                                kind: thir::ExprKind::Literal(thir::Literal::Unit),
                                ty: Ty::UNIT,
                                span: Span::DUMMY,
                            })
                        }),
                    },
                    ty: result_ty,
                    span: Span::DUMMY,
                },
                result_ty,
            )
        }
        Expr::Block { stmts, tail } => {
            let mut block_stmts = Vec::new();
            for &stmt_id in stmts {
                let (stmt_expr, _) = check_expr(chk, body, local_var_map, stmt_id);
                block_stmts.push(thir::Stmt::Expr { expr: stmt_expr });
            }
            if let Some(tail_id) = tail {
                let (tail_expr, tail_ty) = check_expr(chk, body, local_var_map, *tail_id);
                let block_expr = thir::Expr {
                    kind: thir::ExprKind::Block {
                        stmts: block_stmts,
                        tail: Some(Box::new(tail_expr)),
                    },
                    ty: tail_ty,
                    span: Span::DUMMY,
                };
                (block_expr, tail_ty)
            } else {
                let unit_expr = thir::Expr {
                    kind: thir::ExprKind::Block {
                        stmts: block_stmts,
                        tail: None,
                    },
                    ty: Ty::UNIT,
                    span: Span::DUMMY,
                };
                (unit_expr, Ty::UNIT)
            }
        }
        Expr::Missing => {
            tracing::warn!("STUB: encountered Missing expression (unimplemented feature)");
            let unit_expr = thir::Expr {
                kind: thir::ExprKind::Literal(thir::Literal::Unit),
                ty: Ty::UNIT,
                span: Span::DUMMY,
            };
            (unit_expr, Ty::UNIT)
        }
        _ => {
            chk.diagnostics.push(GlyimDiagnostic::type_error(
                Span::DUMMY,
                "unsupported expression".to_string(),
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
        Literal::Int(_, Some(IntTy::I8)) => ctx.mk_ty(TyKind::Int(IntTy::I8)),
        Literal::Int(_, Some(IntTy::I16)) => ctx.mk_ty(TyKind::Int(IntTy::I16)),
        Literal::Int(_, Some(IntTy::I32)) => ctx.mk_ty(TyKind::Int(IntTy::I32)),
        Literal::Int(_, Some(IntTy::I64)) => ctx.mk_ty(TyKind::Int(IntTy::I64)),
        Literal::Int(_, Some(IntTy::Isize)) => ctx.mk_ty(TyKind::Int(IntTy::Isize)),
        Literal::Int(_, None) => ctx.mk_ty(TyKind::Int(IntTy::I32)),
        Literal::Uint(_, Some(UintTy::U8)) => ctx.mk_ty(TyKind::Uint(UintTy::U8)),
        Literal::Uint(_, Some(UintTy::U16)) => ctx.mk_ty(TyKind::Uint(UintTy::U16)),
        Literal::Uint(_, Some(UintTy::U32)) => ctx.mk_ty(TyKind::Uint(UintTy::U32)),
        Literal::Uint(_, Some(UintTy::U64)) => ctx.mk_ty(TyKind::Uint(UintTy::U64)),
        Literal::Uint(_, Some(UintTy::Usize)) => ctx.mk_ty(TyKind::Uint(UintTy::Usize)),
        Literal::Uint(_, None) => ctx.mk_ty(TyKind::Uint(UintTy::U32)),
        Literal::Float(_, FloatTy::F32) => ctx.mk_ty(TyKind::Float(FloatTy::F32)),
        Literal::Float(_, FloatTy::F64) => ctx.mk_ty(TyKind::Float(FloatTy::F64)),

        Literal::Bool(_) => Ty::BOOL,
        Literal::Char(_) => ctx.mk_ty(TyKind::Char),
        Literal::String(_) => ctx.mk_ty(TyKind::String),
        Literal::Unit => Ty::UNIT,
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
