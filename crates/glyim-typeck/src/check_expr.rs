//! Expression checking logic for FnCtxt.

use std::collections::HashMap;

use glyim_core::def_id::{AdtId, FnDefId};
use glyim_core::primitives::*;
use glyim_diag::GlyimDiagnostic;
use glyim_hir::*;
use glyim_type::{GenericArg, Region, Ty, TyKind};

use crate::check_body::FnCtxt;
use crate::thir;
use crate::unify::{literal_ty, thir_literal};

impl<'a> FnCtxt<'a> {
    pub fn check_expr(&mut self, expr_id: ExprId) -> (thir::Expr, Ty) {
        if let Some(cached) = self.expr_cache.get(&expr_id) {
            return (cached.0.clone(), cached.1);
        }

        let expr = &self.body.exprs[expr_id];
        let span = self.body.expr_spans[expr_id];

        let result = match expr {
            Expr::Literal(lit) => {
                let ty = literal_ty(self.ctx, lit);
                (
                    thir::Expr {
                        kind: thir::ExprKind::Literal(thir_literal(lit)),
                        ty,
                        span,
                    },
                    ty,
                )
            }

            Expr::Path(path) => self.check_path(path, span),

            Expr::Block {
                stmts: block_stmts,
                tail,
            } => {
                let mut thir_block_stmts = Vec::new();
                for &stmt_id in block_stmts {
                    let (stmt_expr, _) = self.check_expr(stmt_id);
                    thir_block_stmts.push(thir::Stmt::Expr { expr: stmt_expr });
                }
                if let Some(tail_id) = tail {
                    let (tail_expr, tail_ty) = self.check_expr(*tail_id);
                    let block_expr = thir::Expr {
                        kind: thir::ExprKind::Block {
                            stmts: thir_block_stmts,
                            tail: Some(Box::new(tail_expr)),
                        },
                        ty: tail_ty,
                        span,
                    };
                    (block_expr, tail_ty)
                } else {
                    let unit_expr = thir::Expr {
                        kind: thir::ExprKind::Block {
                            stmts: thir_block_stmts,
                            tail: None,
                        },
                        ty: Ty::UNIT,
                        span,
                    };
                    (unit_expr, Ty::UNIT)
                }
            }

            Expr::Unary { op, expr: operand } => {
                let (inner_expr, inner_ty) = self.check_expr(*operand);
                let result_ty = inner_ty;
                let thir_expr = thir::Expr {
                    kind: thir::ExprKind::Unary {
                        op: *op,
                        operand: Box::new(inner_expr),
                    },
                    ty: result_ty,
                    span,
                };
                (thir_expr, result_ty)
            }

            Expr::Binary { op, lhs, rhs } => {
                let (lhs_expr, lhs_ty) = self.check_expr(*lhs);
                let (_rhs_expr, rhs_ty) = self.check_expr(*rhs);

                let operand_ty = if self.unify(lhs_ty, rhs_ty, span) {
                    lhs_ty
                } else {
                    Ty::ERROR
                };

                let result_ty = match op {
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt => Ty::BOOL,
                    BinOp::And | BinOp::Or => {
                        self.unify(operand_ty, Ty::BOOL, span);
                        Ty::BOOL
                    }
                    _ => operand_ty,
                };

                if result_ty == Ty::ERROR || operand_ty == Ty::ERROR {
                    (thir::Expr::err(span), Ty::ERROR)
                } else {
                    (
                        thir::Expr {
                            kind: thir::ExprKind::Binary {
                                op: *op,
                                lhs: Box::new(lhs_expr),
                                rhs: Box::new(rhs_expr),
                            },
                            ty: result_ty,
                            span,
                        },
                        result_ty,
                    )
                }
            }

            Expr::Ref { expr, mutability } => {
                let (inner_expr, inner_ty) = self.check_expr(*expr);
                let ref_ty = self.ctx.mk_ref(Region::Erased, inner_ty, *mutability);
                (
                    thir::Expr {
                        kind: thir::ExprKind::Ref {
                            mutability: *mutability,
                            operand: Box::new(inner_expr),
                        },
                        ty: ref_ty,
                        span,
                    },
                    ref_ty,
                )
            }

            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let (cond_expr, cond_ty) = self.check_expr(*cond);
                self.unify(cond_ty, Ty::BOOL, span);

                let (then_expr, then_ty) = self.check_expr(*then_branch);

                let (else_opt, else_ty) = if let Some(else_id) = else_branch {
                    let (e, t) = self.check_expr(*else_id);
                    (Some(Box::new(e)), t)
                } else {
                    (None, Ty::UNIT)
                };

                let result_ty = if self.unify(then_ty, else_ty, span) {
                    then_ty
                } else {
                    Ty::ERROR
                };

                (
                    thir::Expr {
                        kind: thir::ExprKind::If {
                            cond: Box::new(cond_expr),
                            then_branch: Box::new(then_expr),
                            else_branch: else_opt,
                        },
                        ty: result_ty,
                        span,
                    },
                    result_ty,
                )
            }

            Expr::While {
                cond,
                body: body_id,
            } => {
                let (cond_expr, cond_ty) = self.check_expr(*cond);
                self.unify(cond_ty, Ty::BOOL, span);
                let (body_expr, _) = self.check_expr(*body_id);
                (
                    thir::Expr {
                        kind: thir::ExprKind::While {
                            cond: Box::new(cond_expr),
                            body: Box::new(body_expr),
                        },
                        ty: Ty::UNIT,
                        span,
                    },
                    Ty::UNIT,
                )
            }

            Expr::Loop { body: body_id } => {
                let (body_expr, _) = self.check_expr(*body_id);
                (
                    thir::Expr {
                        kind: thir::ExprKind::Loop {
                            body: Box::new(body_expr),
                        },
                        ty: Ty::NEVER,
                        span,
                    },
                    Ty::NEVER,
                )
            }

            Expr::For {
                pat,
                iterable,
                body: body_id,
            } => {
                let (_iter_expr, iter_ty) = self.check_expr(*iterable);
                let item_ty = self.fresh_infer_ty();
                let _ = (iter_ty, item_ty);

                self.env.enter_scope();
                let pat_thir = self.check_pattern(*pat, Ty::ERROR);
                self.env.leave_scope();

                self.env.enter_scope();
                let (body_expr, _) = self.check_expr(*body_id);
                self.env.leave_scope();

                (
                    thir::Expr {
                        kind: thir::ExprKind::For {
                            pat: Box::new(pat_thir),
                            iterable: Box::new(thir::Expr::err(span)),
                            body: Box::new(body_expr),
                        },
                        ty: Ty::UNIT,
                        span,
                    },
                    Ty::UNIT,
                )
            }

            Expr::Match { scrutinee, arms } => {
                let (scrut_expr, scrut_ty) = self.check_expr(*scrutinee);
                let result_ty = self.fresh_infer_ty();

                let mut thir_arms = Vec::with_capacity(arms.len());
                for arm in arms {
                    self.env.enter_scope();
                    let pat_thir = self.check_pattern(arm.pat, scrut_ty);
                    let (body_expr, body_ty) = self.check_expr(arm.body);
                    self.env.leave_scope();

                    self.unify(body_ty, result_ty, span);
                    thir_arms.push(thir::MatchArm {
                        pat: pat_thir,
                        guard: None,
                        body: body_expr,
                    });
                }

                (
                    thir::Expr {
                        kind: thir::ExprKind::Match {
                            scrutinee: Box::new(scrut_expr),
                            arms: thir_arms,
                        },
                        ty: result_ty,
                        span,
                    },
                    result_ty,
                )
            }

            Expr::Call { func, args } => {
                let (func_expr, func_ty) = self.check_expr(*func);

                let mut arg_exprs = Vec::with_capacity(args.len());
                for &arg_id in args {
                    arg_exprs.push(self.check_expr(arg_id).0);
                }

                let (is_fn_def, def_id, is_error) = match self.ctx.ty_kind(func_ty) {
                    TyKind::FnDef(def_id, _) => (true, *def_id, false),
                    TyKind::Error => (false, FnDefId::from_raw(0), true),
                    _ => (false, FnDefId::from_raw(0), false),
                };

                let ret_ty = if is_fn_def {
                    self.instantiate_fn_sig(def_id, span)
                } else if is_error {
                    Ty::ERROR
                } else {
                    self.diagnostics.push(GlyimDiagnostic::type_error(
                        span,
                        "call to non-function type",
                    ));
                    Ty::ERROR
                };

                (
                    thir::Expr {
                        kind: thir::ExprKind::Call {
                            func: Box::new(func_expr),
                            args: arg_exprs,
                        },
                        ty: ret_ty,
                        span,
                    },
                    ret_ty,
                )
            }

            Expr::MethodCall {
                receiver,
                method,
                args,
            } => {
                let (recv_expr, _recv_ty) = self.check_expr(*receiver);
                let mut arg_exprs = Vec::new();
                for &arg_id in args {
                    arg_exprs.push(self.check_expr(arg_id).0);
                }

                self.diagnostics.push(GlyimDiagnostic::type_error(
                    span,
                    "method calls are not yet implemented",
                ));
                let _ = (method, recv_expr, arg_exprs);
                (thir::Expr::err(span), Ty::ERROR)
            }

            Expr::Field { receiver, field } => {
                let (_recv_expr, recv_ty) = self.check_expr(*receiver);

                let (is_adt, adt_id, is_tuple) = match self.ctx.ty_kind(recv_ty) {
                    TyKind::Adt(adt_id, _) => (true, *adt_id, false),
                    TyKind::Tuple(_) => (false, AdtId::from_raw(0), true),
                    _ => (false, AdtId::from_raw(0), false),
                };

                let field_ty = if is_adt {
                    self.lookup_field_ty(adt_id, *field, span)
                } else if is_tuple {
                    let idx = self.ctx.name_str(*field).parse::<usize>().ok();
                    if idx.is_some() {
                        self.fresh_infer_ty()
                    } else {
                        self.diagnostics.push(GlyimDiagnostic::type_error(
                            span,
                            format!("no field `{}` on tuple", self.ctx.name_str(*field)),
                        ));
                        Ty::ERROR
                    }
                } else {
                    self.diagnostics.push(GlyimDiagnostic::type_error(
                        span,
                        "field access on non-ADT, non-tuple type",
                    ));
                    Ty::ERROR
                };

                (
                    thir::Expr {
                        kind: thir::ExprKind::Field {
                            receiver: Box::new(recv_expr),
                            field: *field,
                            ty: field_ty,
                        },
                        ty: field_ty,
                        span,
                    },
                    field_ty,
                )
            }

            Expr::Index { base, index } => {
                let (base_expr, base_ty) = self.check_expr(*base);
                let (idx_expr, _idx_ty) = self.check_expr(*index);
                let elem_ty = self.fresh_infer_ty();
                let _ = base_ty;

                (
                    thir::Expr {
                        kind: thir::ExprKind::Index {
                            base: Box::new(base_expr),
                            index: Box::new(idx_expr),
                        },
                        ty: elem_ty,
                        span,
                    },
                    elem_ty,
                )
            }

            Expr::Cast {
                expr,
                ty: target_ref,
            } => {
                let (inner_expr, _inner_ty) = self.check_expr(*expr);
                let target_ty = crate::tyconv::resolve_type_ref(
                    self.ctx,
                    self.infer,
                    self.def_map,
                    self.diagnostics,
                    target_ref,
                    &HashMap::new(),
                    span,
                );
                let result_ty = if target_ty == Ty::ERROR {
                    Ty::ERROR
                } else {
                    target_ty
                };

                (
                    thir::Expr {
                        kind: thir::ExprKind::Cast {
                            expr: Box::new(inner_expr),
                        },
                        ty: result_ty,
                        span,
                    },
                    result_ty,
                )
            }

            Expr::Array(elements) => {
                let elem_ty = self.fresh_infer_ty();
                let mut elem_exprs = Vec::with_capacity(elements.len());
                for &elem_id in elements {
                    let (e_expr, e_ty) = self.check_expr(elem_id);
                    self.unify(e_ty, elem_ty, span);
                    elem_exprs.push(e_expr);
                }
                let arr_ty = self.ctx.mk_ty(TyKind::Slice(elem_ty));
                (
                    thir::Expr {
                        kind: thir::ExprKind::Array(elem_exprs),
                        ty: arr_ty,
                        span,
                    },
                    arr_ty,
                )
            }

            Expr::Tuple(elements) => {
                let mut elem_exprs = Vec::with_capacity(elements.len());
                let mut elem_tys = Vec::with_capacity(elements.len());
                for &elem_id in elements {
                    let (e_expr, e_ty) = self.check_expr(elem_id);
                    elem_exprs.push(e_expr);
                    elem_tys.push(GenericArg::Ty(e_ty));
                }
                let substs = self.ctx.intern_substitution(elem_tys);
                let tup_ty = self.ctx.mk_ty(TyKind::Tuple(substs));
                (
                    thir::Expr {
                        kind: thir::ExprKind::Tuple(elem_exprs),
                        ty: tup_ty,
                        span,
                    },
                    tup_ty,
                )
            }

            Expr::Struct {
                path,
                fields,
                spread,
            } => {
                let _ = (path, spread);
                for field in fields {
                    let _ = self.check_expr(field.1);
                }
                self.diagnostics.push(GlyimDiagnostic::type_error(
                    span,
                    "struct literals are not yet implemented",
                ));
                (thir::Expr::err(span), Ty::ERROR)
            }

            Expr::Assign { lhs, rhs } => {
                let (_lhs_expr, lhs_ty) = self.check_expr(*lhs);
                let (_rhs_expr, rhs_ty) = self.check_expr(*rhs);
                self.unify(rhs_ty, lhs_ty, span);
                let _ = rhs_expr;
                (thir::Expr::err(span), Ty::UNIT)
            }

            Expr::Return { value } => {
                let value_opt = value.map(|val_id| {
                    let (val_expr, val_ty) = self.check_expr(val_id);
                    self.unify(val_ty, self.return_ty, span);
                    val_expr
                });
                (
                    thir::Expr {
                        kind: thir::ExprKind::Break {
                            value: value_opt.map(Box::new),
                        },
                        ty: Ty::NEVER,
                        span,
                    },
                    Ty::NEVER,
                )
            }

            Expr::Break { value } => {
                let value_expr = value.map(|val_id| Box::new(self.check_expr(val_id).0));
                (
                    thir::Expr {
                        kind: thir::ExprKind::Break { value: value_expr },
                        ty: Ty::NEVER,
                        span,
                    },
                    Ty::NEVER,
                )
            }

            Expr::Continue => (
                thir::Expr {
                    kind: thir::ExprKind::Continue,
                    ty: Ty::NEVER,
                    span,
                },
                Ty::NEVER,
            ),

            Expr::Missing => {
                self.diagnostics.push(GlyimDiagnostic::type_error(
                    span,
                    "encountered missing expression",
                ));
                (thir::Expr::err(span), Ty::ERROR)
            }

            _ => {
                self.diagnostics.push(GlyimDiagnostic::type_error(
                    span,
                    "unsupported expression kind",
                ));
                (thir::Expr::err(span), Ty::ERROR)
            }
        };

        self.expr_cache.insert(expr_id, result.clone());
        result
    }
}
