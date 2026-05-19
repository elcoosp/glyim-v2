//! Statement checking logic for FnCtxt.

use glyim_core::interner::Name;
use glyim_core::primitives::Mutability;
use glyim_hir::{Expr, Pat};
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

        // Register parameter bindings from HIR body params (patterns)
        for &pat_id in &self.body.params {
            self.check_param_pattern(pat_id);
        }

        let mut stmts = Vec::new();
        let mut expr_ids: Vec<_> = self.body.exprs.iter_enumerated().collect();
        let len = expr_ids.len();

        for (pos, (expr_id, expr)) in expr_ids.into_iter().enumerate() {
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
                    if is_tail && self.return_ty != Ty::UNIT {
                        self.unify(ty, self.return_ty, span);
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

    /// Check a parameter pattern (from body.params) — adds bindings to env
    /// without requiring a pre-resolved type (uses the param type from the
    /// function signature which was already registered in `check`'s caller).
    fn check_param_pattern(&mut self, pat_id: glyim_hir::PatId) {
        let pat = &self.body.pats[pat_id];
        match pat {
            Pat::Binding {
                name,
                mutability,
                subpattern,
            } => {
                // Use a fresh infer ty — the actual type comes from the
                // param list that was already set up above.
                let ty = self.fresh_infer_ty();
                self.env.add_binding(*name, ty, *mutability);
                if let Some(sub_id) = subpattern {
                    self.check_param_pattern(*sub_id);
                }
            }
            Pat::Wild => {}
            _ => {
                // For other patterns, just recurse — best effort
            }
        }
    }
}
