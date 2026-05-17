//! Statement checking logic for FnCtxt.

use glyim_core::interner::Name;
use glyim_core::primitives::Mutability;
use glyim_hir::Expr;
use glyim_span::Span;
use glyim_type::Ty;

use crate::check_body::FnCtxt;
use crate::thir;

impl<'a> FnCtxt<'a> {
    pub fn check(mut self, params: &[(Name, Ty, Span)]) -> thir::Body {
        let mut thir_params = Vec::with_capacity(params.len());
        for (i, (name, ty, span)) in params.iter().enumerate() {
            let _local_id = thir::LocalVarId::from_raw(i as u32);
            self.env.add_binding(*name, *ty, Mutability::Not);

            thir_params.push(thir::Param {
                name: *name,
                ty: *ty,
                span: *span,
                pat: thir::Pattern::binding(*name, Mutability::Not, *ty, *span),
            });
        }

        let mut stmts = Vec::new();
        let len = self.body.exprs.len();

        for (pos, (expr_id, expr)) in self.body.exprs.iter_enumerated().enumerate() {
            let is_tail = pos == len - 1;
            let span = self.body.expr_spans[expr_id];

            match expr {
                Expr::Assign { lhs, rhs } => {
                    let (lhs_expr, lhs_ty) = self.check_expr(*lhs);
                    let (rhs_expr, rhs_ty) = self.check_expr(*rhs);
                    self.unify(rhs_ty, lhs_ty, span);
                    if is_tail {
                        self.unify(Ty::UNIT, self.return_ty, span);
                    }
                    stmts.push(thir::Stmt::Assign {
                        lhs: lhs_expr,
                        rhs: rhs_expr,
                        span,
                    });
                }
                Expr::Return { value } => {
                    let value_opt = value.map(|val_id| {
                        let (val_expr, val_ty) = self.check_expr(val_id);
                        self.unify(val_ty, self.return_ty, span);
                        val_expr
                    });
                    stmts.push(thir::Stmt::Return {
                        value: value_opt,
                        span,
                    });
                }
                _ => {
                    let (thir_expr, ty) = self.check_expr(expr_id);
                    if is_tail {
                        if self.return_ty != Ty::UNIT {
                            self.unify(ty, self.return_ty, span);
                        }
                    }
                    stmts.push(thir::Stmt::Expr { expr: thir_expr });
                }
            }
        }

        thir::Body {
            owner: self.owner,
            params: thir_params,
            return_ty: self.return_ty,
            stmts,
            span: self.body.span,
        }
    }
}
