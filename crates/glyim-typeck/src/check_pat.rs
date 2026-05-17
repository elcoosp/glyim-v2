//! Pattern checking logic for FnCtxt.

use glyim_diag::GlyimDiagnostic;
use glyim_hir::{Pat, PatId};
use glyim_span::Span;
use glyim_type::Ty;

use crate::check_body::FnCtxt;
use crate::thir;

impl<'a> FnCtxt<'a> {
    pub fn check_pattern(&mut self, pat_id: PatId, expected_ty: Ty) -> thir::Pattern {
        let pat = &self.body.pats[pat_id];
        let span = Span::DUMMY; // Body lacks pat_spans

        match pat {
            Pat::Wild => thir::Pattern::wild(expected_ty, span),
            Pat::Binding {
                name,
                mutability,
                subpattern,
            } => {
                self.env.add_binding(*name, expected_ty, *mutability);
                let sub =
                    subpattern.map(|sub_id| Box::new(self.check_pattern(sub_id, expected_ty)));
                thir::Pattern {
                    kind: thir::PatternKind::Binding {
                        name: *name,
                        mutability: *mutability,
                        subpattern: sub,
                    },
                    ty: expected_ty,
                    span,
                }
            }
            Pat::Tuple(pats) => {
                let mut thir_pats = Vec::with_capacity(pats.len());
                for &p_id in pats {
                    // Without iterating Substitution safely, we fallback to Ty::ERROR for elements
                    thir_pats.push(self.check_pattern(p_id, Ty::ERROR));
                }
                thir::Pattern {
                    kind: thir::PatternKind::Tuple(thir_pats),
                    ty: expected_ty,
                    span,
                }
            }
            _ => {
                self.diagnostics.push(GlyimDiagnostic::type_error(
                    span,
                    "unsupported pattern kind",
                ));
                thir::Pattern::err(span)
            }
        }
    }
}
