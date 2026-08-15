//! Pattern checking logic for FnCtxt.

use glyim_core::def_id::AdtId;
use glyim_core::primitives::Mutability;
use glyim_diag::GlyimDiagnostic;
use glyim_hir::{Pat, PatId};
use glyim_span::Span;
use glyim_type::{Ty, TyKind};

use crate::check_body::FnCtxt;
use crate::thir;


impl<'a> FnCtxt<'a> {
    pub fn check_pattern(&mut self, pat_id: PatId, expected_ty: Ty) -> thir::Pattern {
        let pat = &self.body.pats[pat_id];
        let span = Span::DUMMY;
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
            Pat::Struct { path, fields, rest } => {
                let adt_id = if let Some(name) = path.as_name() {
                    if let Some(res) = self.def_map.modules[self.def_map.root].scope.resolve(name) {
                        AdtId::from_raw(res.0.to_raw())
                    } else {
                        self.diagnostics.push(GlyimDiagnostic::type_error(
                            span,
                            format!("unresolved struct `{}`", self.ctx.name_str(name)),
                        ));
                        return thir::Pattern::err(span);
                    }
                } else {
                    self.diagnostics.push(GlyimDiagnostic::type_error(
                        span,
                        "multi-segment struct paths not yet implemented",
                    ));
                    return thir::Pattern::err(span);
                };
                let mut field_pats = Vec::new();
                for (field_name, field_pat_id) in fields {
                    let field_ty = if self.ctx.adt_def(adt_id).is_some() {
                        self.lookup_field_ty(adt_id, *field_name, span)
                    } else {
                        expected_ty
                    };
                    self.env.add_binding(*field_name, field_ty, Mutability::Not);
                    let field_pat = self.check_pattern(*field_pat_id, field_ty);
                    field_pats.push(thir::FieldPat {
                        field: *field_name,
                        pattern: field_pat,
                        span,
                    });
                }
                thir::Pattern {
                    kind: thir::PatternKind::Struct {
                        adt_id,
                        variant_idx: 0,
                        fields: field_pats,
                        rest: *rest,
                    },
                    ty: expected_ty,
                    span,
                }
            }
            Pat::Tuple(pats) => {
                let mut thir_pats = Vec::new();
                for &p_id in pats {
                    thir_pats.push(self.check_pattern(p_id, Ty::ERROR));
                }
                thir::Pattern {
                    kind: thir::PatternKind::Tuple(thir_pats),
                    ty: expected_ty,
                    span,
                }
            }
            Pat::Literal(lit) => {
                let thir_lit = crate::unify::thir_literal(lit);
                thir::Pattern {
                    kind: thir::PatternKind::Literal(thir_lit),
                    ty: expected_ty,
                    span,
                }
            }
            Pat::Or(pats) => {
                let mut thir_pats = Vec::with_capacity(pats.len());
                let mut first_ty = None;
                for p_id in pats {
                    let sub_pat = self.check_pattern(*p_id, expected_ty);
                    if first_ty.is_none() {
                        first_ty = Some(sub_pat.ty);
                    } else if let Some(ty) = first_ty {
                        // All alternatives must have the same type
                        self.unify(ty, sub_pat.ty, span);
                    }
                    thir_pats.push(sub_pat);
                }
                let unified_ty = first_ty.unwrap_or(expected_ty);
                thir::Pattern {
                    kind: thir::PatternKind::Or(thir_pats),
                    ty: unified_ty,
                    span,
                }
            }
            Pat::Range {
                start,
                end,
                inclusive,
            } => {
                let start_opt = start.as_ref().map(crate::unify::thir_literal);
                let end_opt = end.as_ref().map(crate::unify::thir_literal);
                let ty = expected_ty;
                if ty != Ty::ERROR {
                    if let Some(lit) = start.as_ref() {
                        let lit_ty = crate::unify::literal_ty(self.ctx, lit);
                        self.unify(lit_ty, ty, span);
                    }
                    if let Some(lit) = end.as_ref() {
                        let lit_ty = crate::unify::literal_ty(self.ctx, lit);
                        self.unify(lit_ty, ty, span);
                    }
                }
                thir::Pattern {
                    kind: thir::PatternKind::Range {
                        start: start_opt,
                        end: end_opt,
                        inclusive: *inclusive,
                    },
                    ty,
                    span,
                }
            }
            Pat::Slice(elements) => {
                // Determine element type of the expected array/slice.
                let elem_ty = match self.ctx.ty_kind(expected_ty) {
                    TyKind::Array(ety, _) | TyKind::Slice(ety) => *ety,
                    _ => {
                        self.diagnostics.push(GlyimDiagnostic::type_error(
                            span,
                            "slice pattern requires array or slice type",
                        ));
                        Ty::ERROR
                    }
                };
                if elem_ty == Ty::ERROR {
                    return thir::Pattern::err(span);
                }
                let mut prefix = Vec::new();
                let mut slice_pat = None;
                let mut suffix = Vec::new();
                let mut found_slice = false;
                for &sub_id in elements {
                    let sub = &self.body.pats[sub_id];
                    // A slice‑binding pattern is a wildcard or a simple binding (no subpattern).
                    let is_slice = matches!(
                        sub,
                        Pat::Wild
                            | Pat::Binding {
                                subpattern: None,
                                ..
                            }
                    );
                    if !found_slice && is_slice {
                        // This is the `..` or `rest @ ..` pattern.
                        slice_pat = Some(Box::new(self.check_pattern(sub_id, expected_ty)));
                        found_slice = true;
                    } else if !found_slice {
                        prefix.push(self.check_pattern(sub_id, elem_ty));
                    } else {
                        suffix.push(self.check_pattern(sub_id, elem_ty));
                    }
                }
                // No length check here; it will be done during match lowering.
                thir::Pattern {
                    kind: thir::PatternKind::Slice {
                        prefix,
                        slice: slice_pat,
                        suffix,
                    },
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
