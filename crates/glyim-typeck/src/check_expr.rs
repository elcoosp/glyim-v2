//! Expression checking logic for FnCtxt.

use std::collections::HashMap;

use glyim_core::def_id::{AdtId, FnDefId};
use glyim_core::interner::Name;
use glyim_core::primitives::*;
use glyim_diag::GlyimDiagnostic;
use glyim_hir::*;
use glyim_span::Span;
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
        let span = self.expr_span(expr_id);

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

            Expr::Block { stmts, tail } => {
                let mut thir_stmts = Vec::new();
                for &stmt_id in stmts {
                    let (stmt_expr, _) = self.check_expr(stmt_id);
                    thir_stmts.push(thir::Stmt::Expr { expr: stmt_expr });
                }
                if let Some(tail_id) = tail {
                    let (tail_expr, tail_ty) = self.check_expr(*tail_id);
                    let block_expr = thir::Expr {
                        kind: thir::ExprKind::Block {
                            stmts: thir_stmts,
                            tail: Some(Box::new(tail_expr)),
                        },
                        ty: tail_ty,
                        span,
                    };
                    (block_expr, tail_ty)
                } else {
                    let unit_expr = thir::Expr {
                        kind: thir::ExprKind::Block {
                            stmts: thir_stmts,
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
                let result_ty = match op {
                    UnOp::Neg => {
                        if matches!(
                            self.ctx.ty_kind(inner_ty),
                            TyKind::Int(_) | TyKind::Float(_)
                        ) {
                            inner_ty
                        } else {
                            self.diagnostics.push(GlyimDiagnostic::type_error(
                                span,
                                "cannot apply unary negation to non-numeric type",
                            ));
                            Ty::ERROR
                        }
                    }
                    UnOp::Not => {
                        if matches!(self.ctx.ty_kind(inner_ty), TyKind::Bool | TyKind::Int(_)) {
                            inner_ty
                        } else {
                            self.diagnostics.push(GlyimDiagnostic::type_error(
                                span,
                                "cannot apply unary not to non-bool/non-int type",
                            ));
                            Ty::ERROR
                        }
                    }
                    UnOp::Deref => match self.ctx.ty_kind(inner_ty) {
                        TyKind::Ref(_, inner, _) | TyKind::RawPtr(inner, _) => *inner,
                        _ => {
                            self.diagnostics.push(GlyimDiagnostic::type_error(
                                span,
                                "cannot dereference non-pointer type",
                            ));
                            Ty::ERROR
                        }
                    },
                };
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
                let (rhs_expr, rhs_ty) = self.check_expr(*rhs);

                let operand_ty = if self.unify(lhs_ty, rhs_ty, span) {
                    lhs_ty
                } else {
                    Ty::ERROR
                };

                let result_ty = match op {
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => {
                        Ty::BOOL
                    }
                    BinOp::And | BinOp::Or => {
                        self.unify(operand_ty, Ty::BOOL, span);
                        Ty::BOOL
                    }
                    BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                        if !matches!(
                            self.ctx.ty_kind(operand_ty),
                            TyKind::Int(_) | TyKind::Uint(_)
                        ) && operand_ty != Ty::ERROR
                        {
                            self.diagnostics.push(GlyimDiagnostic::type_error(
                                span,
                                "bitwise operators require integer operands",
                            ));
                        }
                        operand_ty
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

                let result_ty = if then_ty == Ty::ERROR || else_ty == Ty::ERROR {
                    Ty::ERROR
                } else if self.unify(then_ty, else_ty, span) {
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

            Expr::While { cond, body } => {
                let (cond_expr, cond_ty) = self.check_expr(*cond);
                self.unify(cond_ty, Ty::BOOL, span);
                let (body_expr, _) = self.check_expr(*body);
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

            Expr::Loop { body } => {
                let (body_expr, _) = self.check_expr(*body);
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
                body,
            } => {
                let (_iter_expr, _iter_ty) = self.check_expr(*iterable);
                self.env.enter_scope();
                let pat_thir = self.check_pattern(*pat, Ty::ERROR);
                self.env.leave_scope();

                self.env.enter_scope();
                let (body_expr, _) = self.check_expr(*body);
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

                    if body_ty != Ty::ERROR {
                        self.unify(body_ty, result_ty, span);
                    }
                    thir_arms.push(thir::MatchArm {
                        pat: pat_thir,
                        guard: arm.guard.map(|g| Box::new(self.check_expr(g).0)),
                        body: body_expr,
                    });
                }

                let final_ty = if result_ty == Ty::ERROR {
                    Ty::ERROR
                } else if matches!(self.ctx.ty_kind(result_ty), TyKind::Infer(_)) {
                    Ty::UNIT
                } else {
                    result_ty
                };

                (
                    thir::Expr {
                        kind: thir::ExprKind::Match {
                            scrutinee: Box::new(scrut_expr),
                            arms: thir_arms,
                        },
                        ty: final_ty,
                        span,
                    },
                    final_ty,
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
                    TyKind::FnPtr(sig) => {
                        let expected_args = sig.inputs.len() as usize;
                        if args.len() != expected_args && !sig.c_variadic {
                            self.diagnostics.push(GlyimDiagnostic::type_error(
                                span,
                                format!(
                                    "function pointer expects {} arguments, got {}",
                                    expected_args,
                                    args.len()
                                ),
                            ));
                        }
                        return (
                            thir::Expr {
                                kind: thir::ExprKind::Call {
                                    func: Box::new(func_expr),
                                    args: arg_exprs,
                                },
                                ty: sig.output,
                                span,
                            },
                            sig.output,
                        );
                    }
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
                let (recv_expr, recv_ty) = self.check_expr(*receiver);
                let mut arg_exprs = Vec::new();
                for &arg_id in args {
                    arg_exprs.push(self.check_expr(arg_id).0);
                }
                let method_ty = self.resolve_method_call(recv_ty, *method, span);
                let ret_ty = self.extract_return_ty(method_ty, span);
                let thir_expr = thir::Expr {
                    kind: thir::ExprKind::Call {
                        func: Box::new(recv_expr),
                        args: arg_exprs,
                    },
                    ty: ret_ty,
                    span,
                };
                (thir_expr, ret_ty)
            }

            Expr::Field { receiver, field } => {
                let (recv_expr, recv_ty) = self.check_expr(*receiver);

                let field_ty = match self.ctx.ty_kind(recv_ty) {
                    TyKind::Adt(adt_id, substs) => {
                        self.lookup_field_ty_with_substs(*adt_id, *field, span, *substs)
                    }
                    TyKind::Tuple(substs) => {
                        let idx = self.ctx.name_str(*field).parse::<usize>().ok();
                        if let Some(idx) = idx {
                            let args = self.ctx.substitution_args(*substs);
                            if idx < args.len() {
                                if let GenericArg::Ty(ty) = args[idx] {
                                    ty
                                } else {
                                    Ty::ERROR
                                }
                            } else {
                                self.diagnostics.push(GlyimDiagnostic::type_error(
                                    span,
                                    format!(
                                        "tuple index {} out of bounds (length {})",
                                        idx,
                                        args.len()
                                    ),
                                ));
                                Ty::ERROR
                            }
                        } else {
                            self.diagnostics.push(GlyimDiagnostic::type_error(
                                span,
                                format!("no field `{}` on tuple", self.ctx.name_str(*field)),
                            ));
                            Ty::ERROR
                        }
                    }
                    _ => {
                        self.diagnostics.push(GlyimDiagnostic::type_error(
                            span,
                            "field access on non-ADT, non-tuple type",
                        ));
                        Ty::ERROR
                    }
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
                let (idx_expr, idx_ty) = self.check_expr(*index);

                if !matches!(self.ctx.ty_kind(idx_ty), TyKind::Int(_) | TyKind::Uint(_))
                    && idx_ty != Ty::ERROR
                {
                    self.diagnostics.push(GlyimDiagnostic::type_error(
                        span,
                        "index expression must have integer type",
                    ));
                }

                let elem_ty = match self.ctx.ty_kind(base_ty) {
                    TyKind::Array(elem_ty, _) | TyKind::Slice(elem_ty) => *elem_ty,
                    _ => {
                        self.diagnostics.push(GlyimDiagnostic::type_error(
                            span,
                            "indexing operation requires array or slice type",
                        ));
                        self.fresh_infer_ty()
                    }
                };

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
                let (inner_expr, inner_ty) = self.check_expr(*expr);
                let target_ty = crate::tyconv::resolve_type_ref(
                    self.ctx,
                    self.infer,
                    self.def_map,
                    self.diagnostics,
                    target_ref,
                    &HashMap::new(),
                    span,
                );

                if target_ty != Ty::ERROR && inner_ty != Ty::ERROR {
                    if !self.is_cast_valid(inner_ty, target_ty) {
                        self.diagnostics
                            .push(GlyimDiagnostic::type_error(span, "invalid cast"));
                    }
                }

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
                    if e_ty != Ty::ERROR {
                        self.unify(e_ty, elem_ty, span);
                    }
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
                // Resolve the struct type from the path
                let struct_ty = crate::tyconv::resolve_path_type(
                    self.ctx,
                    self.infer,
                    self.def_map,
                    self.diagnostics,
                    path,
                    &HashMap::new(),
                    span,
                );

                if struct_ty == Ty::ERROR {
                    return (thir::Expr::err(span), Ty::ERROR);
                }

                let (adt_id, substs) = match self.ctx.ty_kind(struct_ty) {
                    TyKind::Adt(adt_id, substs) => (*adt_id, *substs),
                    _ => {
                        self.diagnostics.push(GlyimDiagnostic::type_error(
                            span,
                            "struct literal requires ADT type",
                        ));
                        return (thir::Expr::err(span), Ty::ERROR);
                    }
                };

                // Get ADT definition. We extract field info immediately to avoid holding
                // an immutable borrow across mutable self.check_expr calls.
                let adt_def = match self.ctx.adt_def(adt_id) {
                    Some(def) => def,
                    None => {
                        self.diagnostics.push(GlyimDiagnostic::type_error(
                            span,
                            "unknown ADT in struct literal",
                        ));
                        return (thir::Expr::err(span), Ty::ERROR);
                    }
                };

                // Collect field info locally: (name, ty, has_default)
                // This breaks the borrow chain so check_expr can borrow &mut self
                let field_infos: Vec<(Name, Ty)> =
                    adt_def.fields.iter().map(|f| (f.name, f.ty)).collect();

                let mut provided_fields: std::collections::HashSet<Name> =
                    std::collections::HashSet::new();
                let mut thir_fields = Vec::with_capacity(fields.len());

                for &(field_name, field_expr_id) in fields {
                    provided_fields.insert(field_name);

                    // Find field definition in local collection
                    let expected_field_ty =
                        if let Some((_, ty)) = field_infos.iter().find(|(n, _)| *n == field_name) {
                            self.substitute_type(*ty, substs, span)
                        } else {
                            self.diagnostics.push(GlyimDiagnostic::type_error(
                                span,
                                format!("no field `{}` on struct", self.ctx.name_str(field_name)),
                            ));
                            Ty::ERROR
                        };

                    let (field_expr, field_ty) = self.check_expr(field_expr_id);
                    if expected_field_ty != Ty::ERROR && field_ty != Ty::ERROR {
                        self.unify(field_ty, expected_field_ty, span);
                    }

                    // THIR struct fields are tuples: (Name, thir::Expr)
                    thir_fields.push((field_name, field_expr));
                }

                // Handle struct update syntax (spread)
                let spread_expr = if let Some(spread_id) = spread {
                    let (spread_expr, spread_ty) = self.check_expr(*spread_id);
                    if spread_ty != Ty::ERROR {
                        self.unify(spread_ty, struct_ty, span);
                    }
                    Some(Box::new(spread_expr))
                } else {
                    // Check all required fields are provided using local collection
                    for (name, _ty) in &field_infos {
                        if !provided_fields.contains(name) {
                            self.diagnostics.push(GlyimDiagnostic::type_error(
                                span,
                                format!(
                                    "missing field `{}` in struct literal",
                                    self.ctx.name_str(*name)
                                ),
                            ));
                        }
                    }
                    None
                };

                let thir_expr = thir::Expr {
                    kind: thir::ExprKind::Struct {
                        adt_id,
                        fields: thir_fields,
                        spread: spread_expr,
                        variant_idx: 0,
                    },
                    ty: struct_ty,
                    span,
                };

                (thir_expr, struct_ty)
            }

            Expr::Closure { params: _, body: _ } => {
                // Closure lowering is complex; return error with helpful message for now
                self.diagnostics.push(GlyimDiagnostic::type_error(
                    span,
                    "closure type inference not yet fully implemented",
                ));
                (thir::Expr::err(span), Ty::ERROR)
            }

            Expr::Assign { lhs, rhs } => {
                // Assign is handled as a statement-level operation; return unit
                let (_lhs_expr, lhs_ty) = self.check_expr(*lhs);
                let (_rhs_expr, rhs_ty) = self.check_expr(*rhs);
                if lhs_ty != Ty::ERROR && rhs_ty != Ty::ERROR {
                    self.unify(rhs_ty, lhs_ty, span);
                }
                // Return error node since Assign isn't in THIR ExprKind
                (thir::Expr::err(span), Ty::UNIT)
            }

            Expr::Return { value } => {
                let value_opt = value.map(|val_id| {
                    let (val_expr, val_ty) = self.check_expr(val_id);
                    if val_ty != Ty::ERROR && self.return_ty != Ty::ERROR {
                        self.unify(val_ty, self.return_ty, span);
                    }
                    val_expr
                });
                // Return is control flow; THIR uses Break for both
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

            Expr::Range {
                start,
                end,
                inclusive: _,
            } => {
                // Range desugars to struct call; for now return error with helpful message
                if let Some(start_id) = start {
                    let _ = self.check_expr(*start_id);
                }
                if let Some(end_id) = end {
                    let _ = self.check_expr(*end_id);
                }
                self.diagnostics.push(GlyimDiagnostic::type_error(
                    span,
                    "range expressions require std::ops::Range to be in scope",
                ));
                (thir::Expr::err(span), Ty::ERROR)
            }

            Expr::Missing => {
                self.diagnostics.push(GlyimDiagnostic::type_error(
                    span,
                    "encountered missing expression",
                ));
                (thir::Expr::err(span), Ty::ERROR)
            }

            Expr::Err => (thir::Expr::err(span), Ty::ERROR),
        };

        self.expr_cache.insert(expr_id, result.clone());
        result
    }

    // Helper: substitute generic args in a type (simplified)
    fn substitute_type(&self, ty: Ty, substs: glyim_type::Substitution, _span: Span) -> Ty {
        match self.ctx.ty_kind(ty) {
            TyKind::Param(pt) => {
                let args = self.ctx.substitution_args(substs);
                if (pt.index as usize) < args.len() {
                    if let GenericArg::Ty(replacement) = args[pt.index as usize] {
                        return replacement;
                    }
                }
                ty
            }
            _ => ty,
        }
    }

    // Helper: validate cast compatibility
    fn is_cast_valid(&self, from: Ty, to: Ty) -> bool {
        use TyKind::*;
        match (self.ctx.ty_kind(from), self.ctx.ty_kind(to)) {
            (Int(_) | Uint(_), Int(_) | Uint(_) | Float(_)) => true,
            (Float(_), Float(_) | Int(_) | Uint(_)) => true,
            (RawPtr(_, _) | Ref(_, _, _), RawPtr(_, _) | Int(_)) => true,
            (Bool, Int(_) | Uint(_)) => true,
            _ if from == to => true,
            _ => true,
        }
    }

    // Helper for method call resolution
    fn resolve_method_call(&mut self, recv_ty: Ty, method_name: Name, span: Span) -> Ty {
        for (_id, item) in self.hir.items.iter_enumerated() {
            if let glyim_hir::ItemKind::Impl(impl_item) = &item.kind {
                let param_map = crate::tyconv::build_param_tys(self.ctx, &impl_item.generic_params);
                let impl_self_ty = crate::tyconv::resolve_type_ref(
                    self.ctx,
                    self.infer,
                    self.def_map,
                    self.diagnostics,
                    &impl_item.self_ty,
                    &param_map,
                    span,
                );
                if self.unify(recv_ty, impl_self_ty, span) {
                    for method in &impl_item.methods {
                        if method.name == method_name {
                            if let Some(return_ty_ref) = &method.return_ty {
                                return crate::tyconv::resolve_type_ref(
                                    self.ctx,
                                    self.infer,
                                    self.def_map,
                                    self.diagnostics,
                                    return_ty_ref,
                                    &param_map,
                                    span,
                                );
                            } else {
                                return Ty::UNIT;
                            }
                        }
                    }
                }
            }
        }
        self.diagnostics.push(GlyimDiagnostic::type_error(
            span,
            format!(
                "no method `{}` found for type",
                self.ctx.name_str(method_name)
            ),
        ));
        Ty::ERROR
    }

    fn extract_return_ty(&mut self, fn_ty: Ty, _span: Span) -> Ty {
        match self.ctx.ty_kind(fn_ty) {
            TyKind::FnDef(_, _) | TyKind::FnPtr(_) => self.fresh_infer_ty(),
            _ => fn_ty,
        }
    }

    // Helper: lookup field type with generic substitution (needs &mut self for diagnostics)
    fn lookup_field_ty_with_substs(
        &mut self,
        adt_id: AdtId,
        field_name: Name,
        span: Span,
        substs: glyim_type::Substitution,
    ) -> Ty {
        let adt_def = match self.ctx.adt_def(adt_id) {
            Some(def) => def,
            None => {
                self.diagnostics.push(GlyimDiagnostic::type_error(
                    span,
                    "unknown ADT in field lookup",
                ));
                return Ty::ERROR;
            }
        };
        if let Some(field_def) = adt_def.fields.iter().find(|f| f.name == field_name) {
            self.substitute_type(field_def.ty, substs, span)
        } else {
            self.diagnostics.push(GlyimDiagnostic::type_error(
                span,
                format!("no field `{}` on type", self.ctx.name_str(field_name)),
            ));
            Ty::ERROR
        }
    }
}
