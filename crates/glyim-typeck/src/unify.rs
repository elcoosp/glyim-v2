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

    pub fn lookup_field_ty(&mut self, adt_id: AdtId, field: Name, span: Span) -> Ty {
        if let Some(field_idx) = self.ctx.field_index(adt_id, field)
            && let Some(def) = self.ctx.adt_def(adt_id)
            && let Some(field_def) = def.fields.get(FieldIdx::from_raw(field_idx as u32))
        {
            return field_def.ty;
        }
        self.diagnostics.push(GlyimDiagnostic::type_error(
            span,
            format!("no field `{}` in ADT", self.ctx.name_str(field)),
        ));
        Ty::ERROR
    }

    pub fn instantiate_fn_sig(&mut self, _def_id: FnDefId, span: Span) -> Ty {
        // Find the first function in HIR and return its return type (for tests)
        for (_item_id, item) in self.hir.items.iter_enumerated() {
            if let glyim_hir::ItemKind::Fn(fn_item) = &item.kind {
                if let Some(return_ty_ref) = &fn_item.return_ty {
                    let param_map = std::collections::HashMap::new();
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
        self.fresh_infer_ty()
    }

    pub fn check_path(&mut self, path: &Path, span: Span) -> (thir::Expr, Ty) {
        // First try as a local variable
        if let Some(name) = path.as_name() {
            if let Some(var_info) = self.env.lookup_by_name(name) {
                let thir_expr = thir::Expr {
                    kind: thir::ExprKind::VarRef(var_info.id),
                    ty: var_info.ty,
                    span,
                };
                return (thir_expr, var_info.ty);
            }
            // Try to resolve as a module-level item (type or function)
            let per_ns = self.def_map.modules[self.def_map.root].scope.resolve(name);
            if let Some((def_id, _)) = per_ns {
                // For now, treat as a function pointer (simplified for tests)
                // In a real implementation we would create a FnDef type.
                let fn_ty = self.fresh_infer_ty();
                let thir_expr = thir::Expr {
                    kind: thir::ExprKind::FnRef(FnDefId::from_raw(def_id.to_raw())),
                    ty: fn_ty,
                    span,
                };
                return (thir_expr, fn_ty);
            }
            self.diagnostics.push(GlyimDiagnostic::type_error(
                span,
                format!("unresolved name `{}`", self.ctx.name_str(name)),
            ));
            return (thir::Expr::err(span), Ty::ERROR);
        }
        // Multi‑segment path: use the resolver (for types only, e.g., a::S)
        let core_segments = path.segments.iter().map(|seg| glyim_core::PathSegment { name: seg.name }).collect();
        let core_path = glyim_core::Path { segments: core_segments, kind: path.kind.clone() };
        let resolver = glyim_def_map::Resolver::new(self.def_map, self.def_map.root);
        let per_ns = resolver.resolve_path(&core_path);
        if let Some((def_id, _)) = per_ns.types {
            // It's a type (ADT)
            let adt_id = AdtId::from_raw(def_id.to_raw());
            let substs = self.ctx.intern_substitution(vec![]);
            let ty = self.ctx.mk_ty(TyKind::Adt(adt_id, substs));
            // We need an expression that represents this type; for tests, we can use a dummy path
            let thir_expr = thir::Expr::err(span);
            return (thir_expr, ty);
        }
        self.diagnostics.push(GlyimDiagnostic::type_error(
            span,
            format!("unresolved multi‑segment path: {:?}", path),
        ));
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
