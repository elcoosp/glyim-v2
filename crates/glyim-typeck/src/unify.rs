//! Unification and type resolution logic for FnCtxt.

use glyim_core::def_id::{AdtId, ConstDefId, FnDefId};
use glyim_core::interner::Name;
use glyim_core::primitives::{IntTy, UintTy};
use glyim_diag::GlyimDiagnostic;
use glyim_hir::*;
use glyim_span::Span;
use glyim_type::{FieldIdx, FnSig, GenericArg, InferVar, Substitution, Ty, TyCtxMut, TyKind};

use crate::check_body::FnCtxt;
use crate::thir;

impl<'a> FnCtxt<'a> {
    pub fn expr_span(&self, expr_id: ExprId) -> Span {
        if (expr_id.to_raw() as usize) < self.body.expr_spans.len() {
            self.body.expr_spans[expr_id]
        } else {
            Span::DUMMY
        }
    }

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
            let field_ty = field_def.ty;
            return field_ty;
        }
        self.diagnostics.push(GlyimDiagnostic::type_error(
            span,
            format!("no field `{}` in ADT", self.ctx.name_str(field)),
        ));
        Ty::ERROR
    }

    pub fn check_path(&mut self, path: &Path, span: Span) -> (thir::Expr, Ty) {
        // 1. Local variable (already bound in this scope).
        if let Some(name) = path.as_name() {
            if let Some(var_info) = self.env.lookup_by_name(name) {
                self.capture_log.push((var_info.id, var_info.ty, false));
                let thir_expr = thir::Expr {
                    kind: thir::ExprKind::VarRef(var_info.id),
                    ty: var_info.ty,
                    span,
                };
                return (thir_expr, var_info.ty);
            }
        }

        // 2. Value-namespace resolution through the def map (functions, consts,
        //    enum variants). Single- and multi-segment paths both flow through
        //    `Resolver::resolve_path`, which walks the module tree and returns a
        //    `PerNs` with separate type/value namespaces (plan: value paths).
        let core_path = glyim_core::Path {
            segments: path
                .segments
                .iter()
                .map(|s| glyim_core::PathSegment {
                    name: s.name,
                    generic_args: None,
                })
                .collect(),
            kind: path.kind,
        };
        let resolved = {
            let resolver = glyim_def_map::Resolver::new(
                &self.def_map.modules,
                self.def_map.root,
                self.def_map.root,
            );
            resolver.resolve_path(&core_path)
        };

        if let Some((local, _vis)) = resolved.values {
            // Enum variant value path. `Color::Red` (unit) is a value of the
            // enum type; `Some` / `Color::Green` (data-carrying) is a
            // constructor callable as `Some(x)`. The def map registers each
            // variant in the value namespace with a reverse map
            // variant_local -> (enum_local, VariantIdx).
            if let Some((enum_local, variant_idx)) = self.def_map.variant_map.get(&local) {
                let adt_id = AdtId::from_raw(enum_local.to_raw());
                let substs = self.ctx.intern_substitution(vec![]);
                let enum_ty = self.ctx.mk_ty(TyKind::Adt(adt_id, substs));

                // Data-carrying variant (has fields) => a constructor value
                // of function type `fn(field_tys) -> Enum`. Reuse the existing
                // `FnDefId` call machinery by registering a fn-sig for the
                // variant's value `LocalDefId`.
                let has_fields = self
                    .ctx
                    .adt_def(adt_id)
                    .and_then(|def| def.variants.get(variant_idx.index()))
                    .map(|v| !v.fields.is_empty())
                    .unwrap_or(false);

                if has_fields {
                    let ctor_fn_def_id = FnDefId::from_raw(local.to_raw());
                    let field_tys: Vec<Ty> = self
                        .ctx
                        .adt_def(adt_id)
                        .and_then(|def| def.variants.get(variant_idx.index()))
                        .map(|v| v.fields.iter().map(|f| f.ty).collect())
                        .unwrap_or_default();
                    let inputs = self.ctx.intern_substitution(
                        field_tys
                            .iter()
                            .map(|t| GenericArg::Ty(*t))
                            .collect(),
                    );
                    self.ctx.register_fn_sig(
                        ctor_fn_def_id,
                        FnSig {
                            inputs,
                            output: enum_ty,
                            c_variadic: false,
                            unsafety: glyim_core::primitives::Safety::Safe,
                            abi: glyim_core::primitives::Abi::Glyim,
                        },
                    );
                    let fn_ty = self
                        .ctx
                        .mk_ty(TyKind::FnDef(ctor_fn_def_id, substs));
                    let thir_expr = thir::Expr {
                        kind: thir::ExprKind::VariantCtor {
                            adt_id,
                            variant_idx: *variant_idx,
                        },
                        ty: fn_ty,
                        span,
                    };
                    return (thir_expr, fn_ty);
                }

                // Unit variant => a value of the enum type.
                let thir_expr = thir::Expr {
                    kind: thir::ExprKind::VariantRef(adt_id, *variant_idx),
                    ty: enum_ty,
                    span,
                };
                return (thir_expr, enum_ty);
            }

            let fn_def_id = FnDefId::from_raw(local.to_raw());
            if let Some(sig) = self.ctx.fn_sig(fn_def_id) {
                let substs = self.ctx.intern_substitution(vec![]);
                let fn_ty = self.ctx.mk_ty(TyKind::FnDef(fn_def_id, substs));
                let thir_expr = thir::Expr {
                    kind: thir::ExprKind::FnRef(fn_def_id),
                    ty: fn_ty,
                    span,
                };
                return (thir_expr, fn_ty);
            }
            // A constant in the value namespace: emit a `ConstRef` carrying the
            // constant's value type. The def map is the source of truth for the
            // LocalDefId; convert it to a ConstDefId for the THIR node.
            let const_def_id = ConstDefId::from_raw(local.to_raw());
            if let Some(const_ty) = self.ctx.const_ty(const_def_id) {
                let thir_expr = thir::Expr {
                    kind: thir::ExprKind::ConstRef(const_def_id),
                    ty: const_ty,
                    span,
                };
                return (thir_expr, const_ty);
            }
            // Resolved to a value that is neither a registered function nor a
            // registered constant (e.g. an enum variant, which needs a
            // dedicated `VariantRef` THIR node). Report a clear error.
            self.diagnostics.push(GlyimDiagnostic::type_error(
                span,
                "enum-variant value paths are not yet supported".to_string(),
            ));
            return (thir::Expr::err(span), Ty::ERROR);
        }

        // 3. Type-namespace resolution (ADTs, traits) is not a value expression;
        //    fall through to the unresolved-name diagnostic.
        if let Some(name) = path.as_name() {
            self.diagnostics.push(GlyimDiagnostic::type_error(
                span,
                format!("unresolved name `{}`", self.ctx.name_str(name)),
            ));
        } else {
            self.diagnostics.push(GlyimDiagnostic::type_error(
                span,
                "unresolved value path".to_string(),
            ));
        }
        (thir::Expr::err(span), Ty::ERROR)
    }

    pub fn instantiate_fn_sig(&mut self, def_id: FnDefId, span: Span) -> Ty {
        // The function's signature was registered in the type context during
        // crate type-checking (see `register_fn_sig`). Prefer that source of
        // truth for the return type; fall back to scanning HIR items only when
        // no signature was registered (e.g. builtins).
        if let Some(sig) = self.ctx.fn_sig(def_id) {
            return sig.output;
        }
        for (_id, item) in self.hir.items.iter_enumerated() {
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
