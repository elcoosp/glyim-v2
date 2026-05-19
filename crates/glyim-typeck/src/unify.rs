//! Unification and type resolution logic for FnCtxt.

use std::collections::HashMap;

use glyim_core::def_id::{AdtId, FnDefId};
use glyim_core::interner::Name;
use glyim_core::primitives::{IntTy, UintTy};
use glyim_diag::GlyimDiagnostic;
use glyim_hir::*;
use glyim_span::Span;
use glyim_type::{FieldIdx, InferVar, Ty, TyCtxMut, TyKind};

use crate::check_body::FnCtxt;
use crate::thir;

impl<'a> FnCtxt<'a> {
    pub fn fresh_infer_ty(&mut self) -> Ty {
        let var = self.infer.new_ty_var(self.ctx);
        self.ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
    }

    pub fn unify(&mut self, a: Ty, b: Ty, span: Span) -> bool {
        if a == Ty::ERROR || b == Ty::ERROR {
            return false;
        }
        match self.infer.unify(self.ctx, a, b, span) {
            Ok(_) => true,
            Err(diags) => {
                self.diagnostics.extend(diags);
                false
            }
        }
    }

    /// Get the span for an expression, with a fallback for missing spans.
    pub fn expr_span(&self, expr_id: ExprId) -> Span {
        if (expr_id.to_raw() as usize) < self.body.expr_spans.len() {
            self.body.expr_spans[expr_id]
        } else {
            Span::DUMMY
        }
    }

    pub fn lookup_field_ty(&mut self, adt_id: AdtId, field: Name, span: Span) -> Ty {
        if let Some(field_idx) = self.ctx.field_index(adt_id, field)
            && let Some(def) = self.ctx.adt_def(adt_id)
            && let Some(field_def) = def.fields.get(FieldIdx::from_raw(field_idx as u32))
        {
            let field_ty = field_def.ty;
            return field_ty;
        }

        // Fallback: look up field type from HIR
        if let Some(ty) = self.lookup_field_ty_from_hir(adt_id, field) {
            return ty;
        }

        self.diagnostics.push(GlyimDiagnostic::type_error(
            span,
            format!("no field `{}` in ADT", self.ctx.name_str(field)),
        ));
        Ty::ERROR
    }

    /// Look up a field's type from the HIR struct definition.
    fn lookup_field_ty_from_hir(&mut self, adt_id: AdtId, field_name: Name) -> Option<Ty> {
        for (_id, item) in self.hir.items.iter_enumerated() {
            if let glyim_hir::ItemKind::Struct(struct_item) = &item.kind {
                if let Some(res) = self.def_map.modules[self.def_map.root].scope.resolve(item.name)
                {
                    if AdtId::from_raw(res.0.to_raw()) == adt_id {
                        for field in &struct_item.fields {
                            if field.name == field_name {
                                let param_map =
                                    crate::tyconv::build_param_tys(self.ctx, &struct_item.generic_params);
                                return Some(crate::tyconv::resolve_type_ref(
                                    self.ctx,
                                    self.infer,
                                    self.def_map,
                                    self.diagnostics,
                                    &field.ty,
                                    &param_map,
                                    Span::DUMMY,
                                ));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn instantiate_fn_sig(&mut self, _def_id: FnDefId, span: Span) -> Ty {
        // Try to find the function's return type in the HIR.
        // We search all fn items and impl methods; the first match
        // with a return type is used (refined later with proper DefId mapping).
        for (_id, item) in self.hir.items.iter_enumerated() {
            if let glyim_hir::ItemKind::Fn(fn_item) = &item.kind {
                if let Some(return_ty_ref) = &fn_item.return_ty {
                    let param_map = HashMap::new();
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
        // Also check impl methods
        for (_id, item) in self.hir.items.iter_enumerated() {
            if let glyim_hir::ItemKind::Impl(impl_item) = &item.kind {
                for method in &impl_item.methods {
                    if let Some(return_ty_ref) = &method.return_ty {
                        let param_map = crate::tyconv::build_param_tys(
                            self.ctx,
                            &impl_item.generic_params,
                        );
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
        self.fresh_infer_ty()
    }

    pub fn check_path(&mut self, path: &Path, span: Span) -> (thir::Expr, Ty) {
        // Single-segment: try local variable first
        if let Some(name) = path.as_name() {
            if let Some(var_info) = self.env.lookup_by_name(name) {
                return (
                    thir::Expr {
                        kind: thir::ExprKind::VarRef(var_info.id),
                        ty: var_info.ty,
                        span,
                    },
                    var_info.ty,
                );
            }
            // Not a local — fall through to def map resolution
        }

        // Try to resolve as a type or value path through the def map
        let core_segments = path
            .segments
            .iter()
            .map(|seg| glyim_core::PathSegment {
                name: seg.name,
            })
            .collect();
        let core_path = glyim_core::Path {
            segments: core_segments,
            kind: path.kind.clone(),
        };
        let resolver = glyim_def_map::Resolver::new(self.def_map, self.def_map.root);
        let per_ns = resolver.resolve_path(&core_path);

        // Try value namespace (functions)
        if let Some((def_id, _vis)) = per_ns.values {
            let fn_def_id = FnDefId::from_raw(def_id.to_raw());
            let substs = self.ctx.intern_substitution(vec![]);
            let fn_ty = self.ctx.mk_ty(TyKind::FnDef(fn_def_id, substs));
            return (
                thir::Expr {
                    kind: thir::ExprKind::FnRef(fn_def_id),
                    ty: fn_ty,
                    span,
                },
                fn_ty,
            );
        }

        // Try type namespace (structs, etc.)
        if let Some((def_id, _vis)) = per_ns.types {
            let adt_id = AdtId::from_raw(def_id.to_raw());
            let substs = self.ctx.intern_substitution(vec![]);
            let ty = self.ctx.mk_ty(TyKind::Adt(adt_id, substs));
            return (
                thir::Expr {
                    kind: thir::ExprKind::Err,
                    ty,
                    span,
                },
                ty,
            );
        }

        // Error
        if let Some(name) = path.as_name() {
            self.diagnostics.push(GlyimDiagnostic::type_error(
                span,
                format!("unresolved name `{}`", self.ctx.name_str(name)),
            ));
        } else {
            self.diagnostics.push(GlyimDiagnostic::type_error(
                span,
                format!(
                    "unresolved path: {}",
                    path.segments
                        .iter()
                        .map(|s| self.ctx.name_str(s.name))
                        .collect::<Vec<_>>()
                        .join("::")
                ),
            ));
        }
        (thir::Expr::err(span), Ty::ERROR)
    }
}

pub fn literal_ty(ctx: &mut TyCtxMut, lit: &Literal) -> Ty {
    match lit {
        Literal::Int(_, Some(hint)) => ctx.mk_ty(TyKind::Int(*hint)),
        Literal::Int(_, None) => ctx.mk_ty(TyKind::Int(IntTy::I32)),
        Literal::Uint(_, Some(hint)) => ctx.mk_ty(TyKind::Uint(*hint)),
        Literal::Uint(_, None) => ctx.mk_ty(TyKind::Uint(UintTy::U32)),
        Literal::Float(_, ft) => ctx.mk_ty(TyKind::Float(*ft)),
        Literal::Bool(_) => Ty::BOOL,
        Literal::Char(_) => ctx.mk_ty(TyKind::Char),
        Literal::String(_) => ctx.mk_ty(TyKind::String),
        Literal::Unit => Ty::UNIT,
    }
}

pub fn thir_literal(lit: &Literal) -> thir::Literal {
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
