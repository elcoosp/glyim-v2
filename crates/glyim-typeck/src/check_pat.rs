//! Pattern checking logic for FnCtxt.

use std::collections::HashMap;

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
            Pat::Struct {
                path,
                fields,
                rest,
            } => {
                // Resolve the struct path to get the AdtId
                let adt_id = if let Some(name) = path.as_name() {
                    if let Some(res) =
                        self.def_map.modules[self.def_map.root].scope.resolve(name)
                    {
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
                        "multi-segment struct paths not yet implemented in patterns",
                    ));
                    return thir::Pattern::err(span);
                };

                // Collect field types: try TyCtx first, then HIR
                let field_type_map = self.collect_struct_field_types(adt_id);

                // Check each field pattern
                let mut field_pats = Vec::with_capacity(fields.len());
                for (field_name, field_pat_id) in fields {
                    let field_ty = field_type_map
                        .get(field_name)
                        .copied()
                        .unwrap_or_else(|| self.fresh_infer_ty());

                    // Add binding for this field to the environment
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
                let mut thir_pats = Vec::with_capacity(pats.len());
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
            _ => {
                self.diagnostics.push(GlyimDiagnostic::type_error(
                    span,
                    "unsupported pattern kind",
                ));
                thir::Pattern::err(span)
            }
        }
    }

    /// Collect struct field types by trying TyCtx first, then HIR.
    fn collect_struct_field_types(&mut self, adt_id: AdtId) -> HashMap<glyim_core::interner::Name, Ty> {
        // Look up from HIR directly (TyCtx may not have field name mapping)
        for (_id, item) in self.hir.items.iter_enumerated() {
            if let glyim_hir::ItemKind::Struct(struct_item) = &item.kind {
                if let Some(res) = self.def_map.modules[self.def_map.root].scope.resolve(item.name) {
                    if AdtId::from_raw(res.0.to_raw()) == adt_id {
                        let mut map = HashMap::new();
                        let param_map = crate::tyconv::build_param_tys(self.ctx, &struct_item.generic_params);
                        for field in &struct_item.fields {
                            let field_ty = crate::tyconv::resolve_type_ref(
                                self.ctx,
                                self.infer,
                                self.def_map,
                                self.diagnostics,
                                &field.ty,
                                &param_map,
                                Span::DUMMY,
                            );
                            map.insert(field.name, field_ty);
                        }
                        return map;
                    }
                }
            }
        }
        HashMap::new()
    }
            }
            if !map.is_empty() {
                return map;
            }
        }

        // Fallback: look up from HIR
        for (_id, item) in self.hir.items.iter_enumerated() {
            if let glyim_hir::ItemKind::Struct(struct_item) = &item.kind {
                if let Some(res) =
                    self.def_map.modules[self.def_map.root].scope.resolve(item.name)
                {
                    if AdtId::from_raw(res.0.to_raw()) == adt_id {
                        let mut map = HashMap::new();
                        let param_map =
                            crate::tyconv::build_param_tys(self.ctx, &struct_item.generic_params);
                        for field in &struct_item.fields {
                            let field_ty = crate::tyconv::resolve_type_ref(
                                self.ctx,
                                self.infer,
                                self.def_map,
                                self.diagnostics,
                                &field.ty,
                                &param_map,
                                Span::DUMMY,
                            );
                            map.insert(field.name, field_ty);
                        }
                        return map;
                    }
                }
            }
        }

        HashMap::new()
    }
}
