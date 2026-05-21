//! Pattern checking logic for FnCtxt.

use glyim_core::def_id::AdtId;
use glyim_core::primitives::Mutability;
use glyim_diag::GlyimDiagnostic;
use glyim_hir::{Pat, PatId};
use glyim_span::Span;
use glyim_type::Ty;

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
                        Pat::Slice(elements) => {
                // Determine element type of scrutinee (must be array or slice)
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
                // Scan elements to find the slice matcher (Wild or Binding without subpattern)
                let mut prefix = Vec::new();
                let mut slice_pat = None;
                let mut suffix = Vec::new();
                let mut found_slice = false;
                for p_id in elements {
                    let pat = &self.body.pats[*p_id];
                    let is_slice_pat = match pat {
                        Pat::Wild => true,
                        Pat::Binding { subpattern: None, .. } => true,
                        _ => false,
                    };
                    if !found_slice && is_slice_pat {
                        slice_pat = Some(*p_id);
                        found_slice = true;
                    } else if !found_slice {
                        prefix.push(*p_id);
                    } else {
                        suffix.push(*p_id);
                    }
                }
                // Type-check subpatterns
                for p_id in &prefix {
                    self.check_pattern(*p_id, elem_ty);
                }
                if let Some(p_id) = slice_pat {
                    self.check_pattern(p_id, expected_ty);
                }
                for p_id in &suffix {
                    self.check_pattern(*p_id, elem_ty);
                }
                // Build THIR pattern (placeholder with empty vectors; actual vectors would need to carry checked patterns)
                thir::Pattern {
                    kind: thir::PatternKind::Slice {
                        prefix: vec![],
                        slice: None,
                        suffix: vec![],
                    },
                    ty: expected_ty,
                    span,
                }
            }
                let thir_lit = crate::unify::thir_literal(lit);
                thir::Pattern {
                    kind: thir::PatternKind::Literal(thir_lit),
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
