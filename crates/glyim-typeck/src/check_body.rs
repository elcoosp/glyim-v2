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
    local_var_types: std::collections::HashMap<Name, Ty>,
}

#[allow(clippy::too_many_arguments, unused_assignments)]
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

    let mut local_var_types = std::collections::HashMap::new();
    for (i, param) in params.iter().enumerate() {
        let ty = thir_params[i].ty;
        local_var_types.insert(param.name, ty);
    }

    let mut chk = CheckCtx {
        ctx,
        infer,
        diagnostics,
        pending_obligations,
        hir,
        local_var_types,
    };

    let mut thir_stmts = Vec::new();
    let mut _tail_expr_ty: Option<Ty> = None;
    let len = body.exprs.len();
    for (pos, (expr_id, expr)) in body.exprs.iter_enumerated().enumerate() {
        let is_tail = pos == len - 1;

        match expr {
            Expr::Assign { lhs, rhs } => {
                let (lhs_expr, _lhs_ty) = check_expr(&mut chk, body, &local_var_map, *lhs);
                let (rhs_expr, _rhs_ty) = check_expr(&mut chk, body, &local_var_map, *rhs);
                thir_stmts.push(thir::Stmt::Assign {
                    lhs: lhs_expr,
                    rhs: rhs_expr,
                    span: Span::DUMMY,
                });
                if is_tail {
                    _tail_expr_ty = Some(Ty::UNIT);
                }
            }
            Expr::Return { value } => {
                let value_opt = if let Some(val_id) = value {
                    let (val_expr, _val_ty) = check_expr(&mut chk, body, &local_var_map, *val_id);
                    Some(val_expr)
                } else {
                    None
                };
                thir_stmts.push(thir::Stmt::Return {
                    value: value_opt,
                    span: Span::DUMMY,
                });
            }
            Expr::Break { .. } => {
                let (thir_br, _ty) = check_expr(&mut chk, body, &local_var_map, expr_id);
                thir_stmts.push(thir::Stmt::Expr { expr: thir_br });
                if is_tail {
                    _tail_expr_ty = Some(Ty::NEVER);
                }
            }
            Expr::Continue => {
                let (thir_cont, _ty) = check_expr(&mut chk, body, &local_var_map, expr_id);
                thir_stmts.push(thir::Stmt::Expr { expr: thir_cont });
                if is_tail {
                    _tail_expr_ty = Some(Ty::NEVER);
                }
            }
            _ => {
                let (thir_expr, expr_ty) = check_expr(&mut chk, body, &local_var_map, expr_id);
                if is_tail {
                    if return_ty != Ty::UNIT {
                        let span = body.span;
                        if let Err(diags) = chk.infer.unify(chk.ctx, expr_ty, return_ty, span) {
                            chk.diagnostics.extend(diags);
                        }
                    }
                    _tail_expr_ty = Some(expr_ty);
                } else {
                    thir_stmts.push(crate::thir::Stmt::Expr { expr: thir_expr });
                }
            }
        }
    }
    let _ = _tail_expr_ty;

    thir::Body {
        owner: glyim_core::def_id::DefId::new(CrateId::from_raw(0), local_def_id),
        params: thir_params,
        return_ty,
        stmts: thir_stmts,
        span: body.span,
    }
}

fn fresh_infer_ty(chk: &mut CheckCtx) -> Ty {
    let var = chk.infer.new_ty_var(chk.ctx);
    chk.ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
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
            if let Some(name) = path.as_name() {
                if let Some(&ty) = chk.local_var_types.get(&name) {
                    let local_id = local_var_map
                        .get(&name)
                        .copied()
                        .unwrap_or_else(|| LocalVarId::from_raw(0));
                    let thir_expr = thir::Expr {
                        kind: thir::ExprKind::VarRef(local_id),
                        ty,
                        span: Span::DUMMY,
                    };
                    return (thir_expr, ty);
                }
                if let Some(&local_id) = local_var_map.get(&name) {
                    let idx = local_id.to_raw() as usize;
                    let param_ty = if idx < body.params.len() {
                        fresh_infer_ty(chk)
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
            if let Err(diags) = chk.infer.unify(chk.ctx, lhs_ty, rhs_ty, Span::DUMMY) {
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
            if let Err(diags) = chk.infer.unify(chk.ctx, cond_ty, Ty::BOOL, Span::DUMMY) {
                chk.diagnostics.extend(diags);
            }
            let (then_expr, then_ty) = check_expr(chk, body, local_var_map, *then_branch);
            if let Some(else_id) = else_branch {
                let (_else_expr, else_ty) = check_expr(chk, body, local_var_map, *else_id);
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
        Expr::While {
            cond,
            body: body_id,
        } => {
            let (cond_expr, cond_ty) = check_expr(chk, body, local_var_map, *cond);
            if let Err(diags) = chk.infer.unify(chk.ctx, cond_ty, Ty::BOOL, Span::DUMMY) {
                chk.diagnostics.extend(diags);
            }
            let (body_expr, _) = check_expr(chk, body, local_var_map, *body_id);
            (
                thir::Expr {
                    kind: thir::ExprKind::While {
                        cond: Box::new(cond_expr),
                        body: Box::new(body_expr),
                    },
                    ty: Ty::UNIT,
                    span: Span::DUMMY,
                },
                Ty::UNIT,
            )
        }
        Expr::Loop { body: body_id } => {
            let (body_expr, _) = check_expr(chk, body, local_var_map, *body_id);
            (
                thir::Expr {
                    kind: thir::ExprKind::Loop {
                        body: Box::new(body_expr),
                    },
                    ty: Ty::NEVER,
                    span: Span::DUMMY,
                },
                Ty::NEVER,
            )
        }
        Expr::For {
            pat: _,
            iterable,
            body: body_id,
        } => {
            // Ignore pattern for now, infer fresh type
            let pat_ty = fresh_infer_ty(chk);
            let (_iter_expr, _) = check_expr(chk, body, local_var_map, *iterable);
            let (body_expr, _) = check_expr(chk, body, local_var_map, *body_id);
            (
                thir::Expr {
                    kind: thir::ExprKind::For {
                        pat: Box::new(thir::Pattern {
                            kind: thir::PatternKind::Binding {
                                name: glyim_core::interner::Interner::new().intern("for_pat"),
                                mutability: Mutability::Not,
                                subpattern: None,
                            },
                            ty: pat_ty,
                            span: Span::DUMMY,
                        }),
                        iterable: Box::new(_iter_expr),
                        body: Box::new(body_expr),
                    },
                    ty: Ty::UNIT,
                    span: Span::DUMMY,
                },
                Ty::UNIT,
            )
        }
        Expr::Match { scrutinee, arms } => {
            let (scrut_expr, _) = check_expr(chk, body, local_var_map, *scrutinee);
            let result_ty = fresh_infer_ty(chk);
            let mut thir_arms = Vec::new();
            for arm in arms {
                let (arm_body_expr, arm_body_ty) = check_expr(chk, body, local_var_map, arm.body);
                if let Err(diags) = chk
                    .infer
                    .unify(chk.ctx, arm_body_ty, result_ty, Span::DUMMY)
                {
                    chk.diagnostics.extend(diags);
                }
                thir_arms.push(thir::MatchArm {
                    pat: thir::Pattern {
                        kind: thir::PatternKind::Wild,
                        ty: Ty::UNIT,
                        span: Span::DUMMY,
                    },
                    guard: None,
                    body: arm_body_expr,
                });
            }
            (
                thir::Expr {
                    kind: thir::ExprKind::Match {
                        scrutinee: Box::new(scrut_expr),
                        arms: thir_arms,
                    },
                    ty: result_ty,
                    span: Span::DUMMY,
                },
                result_ty,
            )
        }
        Expr::Call { func, args } => {
            let (func_expr, _) = check_expr(chk, body, local_var_map, *func);
            let mut thir_args = Vec::new();
            for &arg_id in args {
                let (arg_expr, _) = check_expr(chk, body, local_var_map, arg_id);
                thir_args.push(arg_expr);
            }
            let ret_ty = fresh_infer_ty(chk);
            (
                thir::Expr {
                    kind: thir::ExprKind::Call {
                        func: Box::new(func_expr),
                        args: thir_args,
                    },
                    ty: ret_ty,
                    span: Span::DUMMY,
                },
                ret_ty,
            )
        }
        Expr::MethodCall {
            receiver: _,
            method: _,
            args: _,
        } => {
            // FIXME: method calls not implemented
            unimplemented!("method calls not implemented");
        }
        Expr::Field { receiver, field } => {
            let (recv_expr, _) = check_expr(chk, body, local_var_map, *receiver);
            let field_ty = fresh_infer_ty(chk);
            (
                thir::Expr {
                    kind: thir::ExprKind::Field {
                        receiver: Box::new(recv_expr),
                        field: *field,
                        ty: field_ty,
                    },
                    ty: field_ty,
                    span: Span::DUMMY,
                },
                field_ty,
            )
        }
        Expr::Index { base, index } => {
            let (base_expr, _) = check_expr(chk, body, local_var_map, *base);
            let (idx_expr, _) = check_expr(chk, body, local_var_map, *index);
            let elem_ty = fresh_infer_ty(chk);
            (
                thir::Expr {
                    kind: thir::ExprKind::Index {
                        base: Box::new(base_expr),
                        index: Box::new(idx_expr),
                    },
                    ty: elem_ty,
                    span: Span::DUMMY,
                },
                elem_ty,
            )
        }
        Expr::Cast {
            expr,
            ty: _type_ref,
        } => {
            let (inner_expr, _) = check_expr(chk, body, local_var_map, *expr);
            let target_ty = fresh_infer_ty(chk);
            (
                thir::Expr {
                    kind: thir::ExprKind::Cast {
                        expr: Box::new(inner_expr),
                    },
                    ty: target_ty,
                    span: Span::DUMMY,
                },
                target_ty,
            )
        }
        Expr::Array(elements) => {
            let mut elem_exprs = Vec::new();
            let elem_ty = fresh_infer_ty(chk);
            for &elem_id in elements {
                let (e_expr, e_ty) = check_expr(chk, body, local_var_map, elem_id);
                if let Err(diags) = chk.infer.unify(chk.ctx, e_ty, elem_ty, Span::DUMMY) {
                    chk.diagnostics.extend(diags);
                }
                elem_exprs.push(e_expr);
            }
            let arr_ty = chk.ctx.mk_ty(TyKind::Slice(elem_ty));
            (
                thir::Expr {
                    kind: thir::ExprKind::Array(elem_exprs),
                    ty: arr_ty,
                    span: Span::DUMMY,
                },
                arr_ty,
            )
        }
        Expr::Tuple(elements) => {
            let mut tuple_exprs = Vec::new();
            for &elem_id in elements {
                let (e_expr, _) = check_expr(chk, body, local_var_map, elem_id);
                tuple_exprs.push(e_expr);
            }
            let tup_ty = fresh_infer_ty(chk);
            (
                thir::Expr {
                    kind: thir::ExprKind::Tuple(tuple_exprs),
                    ty: tup_ty,
                    span: Span::DUMMY,
                },
                tup_ty,
            )
        }
        Expr::Struct {
            path: _,
            fields: _,
            spread: _,
        } => {
            // FIXME: struct literals not implemented
            unimplemented!("struct literals not implemented");
        }
        Expr::Break { value } => {
            let value_expr = if let Some(val_id) = value {
                let (val_expr, _) = check_expr(chk, body, local_var_map, *val_id);
                Some(Box::new(val_expr))
            } else {
                None
            };
            (
                thir::Expr {
                    kind: thir::ExprKind::Break { value: value_expr },
                    ty: Ty::NEVER,
                    span: Span::DUMMY,
                },
                Ty::NEVER,
            )
        }
        Expr::Continue => (
            thir::Expr {
                kind: thir::ExprKind::Continue,
                ty: Ty::NEVER,
                span: Span::DUMMY,
            },
            Ty::NEVER,
        ),
        Expr::Missing => {
            // FIXME: encountered Missing expression (unimplemented feature)
            unimplemented!("encountered Missing expression (unimplemented feature)");
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
