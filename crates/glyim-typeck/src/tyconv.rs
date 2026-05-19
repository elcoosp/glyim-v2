//! HIR `TypeRef` → `Ty` conversion.

use std::collections::HashMap;

use glyim_core::def_id::{AdtId, DefId, TraitDefId};
use glyim_core::interner::Name;
use glyim_core::primitives::*;
use glyim_core::Path as CorePath;
use glyim_diag::GlyimDiagnostic;
use glyim_solve::InferenceTable;
use glyim_span::Span;
use glyim_type::*;
use glyim_def_map::Resolver;

use crate::coherence::ResolvedImplHeader;

pub fn resolve_type_ref(
    ctx: &mut TyCtxMut,
    infer: &mut InferenceTable,
    def_map: &glyim_def_map::CrateDefMap,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    ty_ref: &glyim_hir::TypeRef,
    param_map: &HashMap<Name, Ty>,
    span: Span,
) -> Ty {
    match ty_ref {
        glyim_hir::TypeRef::Path(path) => {
            resolve_path_type(ctx, infer, def_map, diagnostics, path, param_map, span)
        }
        glyim_hir::TypeRef::Ref { inner, mutability } => {
            let inner_ty = resolve_type_ref(ctx, infer, def_map, diagnostics, inner, param_map, span);
            if inner_ty == Ty::ERROR { return Ty::ERROR; }
            ctx.mk_ref(Region::Erased, inner_ty, *mutability)
        }
        glyim_hir::TypeRef::Tuple(elements) => {
            let mut tys = Vec::new();
            for elem in elements {
                tys.push(resolve_type_ref(ctx, infer, def_map, diagnostics, elem, param_map, span));
            }
            if tys.is_empty() { return Ty::UNIT; }
            if tys.contains(&Ty::ERROR) { return Ty::ERROR; }
            let substs = ctx.intern_substitution(tys.into_iter().map(GenericArg::Ty).collect());
            ctx.mk_ty(TyKind::Tuple(substs))
        }
        glyim_hir::TypeRef::Never => Ty::NEVER,
        glyim_hir::TypeRef::Infer => {
            let var = infer.new_ty_var(ctx);
            ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
        }
        glyim_hir::TypeRef::Error => Ty::ERROR,
        _ => {
            diagnostics.push(GlyimDiagnostic::type_error(span, "unsupported type annotation".to_string()));
            Ty::ERROR
        }
    }
}

pub fn build_param_tys(ctx: &mut TyCtxMut, params: &[glyim_hir::GenericParam]) -> HashMap<Name, Ty> {
    let mut map = HashMap::new();
    for (i, param) in params.iter().enumerate() {
        let pt = ParamTy { index: i as u32, name: param.name };
        map.insert(param.name, ctx.mk_ty(TyKind::Param(pt)));
    }
    map
}

#[derive(Clone, Debug)]
pub struct FnSig {
    pub param_tys: Vec<Ty>,
    pub return_ty: Ty,
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_fn_sig(
    ctx: &mut TyCtxMut,
    infer: &mut InferenceTable,
    def_map: &glyim_def_map::CrateDefMap,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    params: &[glyim_hir::Param],
    return_ty_ref: &Option<glyim_hir::TypeRef>,
    generic_params: &[glyim_hir::GenericParam],
    span: Span,
) -> FnSig {
    let param_map = build_param_tys(ctx, generic_params);
    let mut param_tys = Vec::new();
    for param in params {
        let ty = if let Some(ty_ref) = &param.ty {
            let resolved = resolve_type_ref(ctx, infer, def_map, diagnostics, ty_ref, &param_map, param.span);
            if resolved == Ty::ERROR {
                let var = infer.new_ty_var(ctx);
                ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
            } else { resolved }
        } else {
            let var = infer.new_ty_var(ctx);
            ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
        };
        param_tys.push(ty);
    }
    let return_ty = match return_ty_ref {
        Some(ty_ref) => {
            let resolved = resolve_type_ref(ctx, infer, def_map, diagnostics, ty_ref, &param_map, span);
            if resolved == Ty::ERROR {
                let var = infer.new_ty_var(ctx);
                ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
            } else { resolved }
        }
        None => Ty::UNIT,
    };
    FnSig { param_tys, return_ty }
}

pub fn resolve_impl_header(infer: pub fn resolve_impl_header(mut InferenceTable, infer: pub fn resolve_impl_header(mut InferenceTable, 
    ctx: &mut TyCtxMut,
    infer: infer: infer: &mut InferenceTablemut InferenceTablemut InferenceTable,
    def_map: &glyim_def_map::CrateDefMap,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    impl_item: &glyim_hir::ImplItem,
    span: Span,
) -> ResolvedImplHeader {
    let param_map = build_param_tys(ctx, &impl_item.generic_params);
    let self_ty = resolve_type_ref(ctx, infer, def_map, diagnostics, &impl_item.self_ty, &param_map, span);
    let (trait_def_id, trait_name, trait_substs) = match &impl_item.trait_ref {
        Some(path) => {
            if let Some(name) = path.as_name() {
                match resolve_name_to_def_id(def_map, name) {
                    Some(def_id) => {
                        let trait_def_id = TraitDefId::from_raw(def_id.local_id.to_raw());
                        (Some(trait_def_id), Some(name), ctx.intern_substitution(vec![]))
                    }
                    None => {
                        diagnostics.push(GlyimDiagnostic::type_error(span, format!("unresolved trait `{}`", def_map.interner.resolve(name))));
                        (None, Some(name), ctx.intern_substitution(vec![]))
                    }
                }
            } else {
                diagnostics.push(GlyimDiagnostic::type_error(span, "multi-segment trait paths not yet implemented".to_string()));
                (None, None, ctx.intern_substitution(vec![]))
            }
        }
        None => (None, None, ctx.intern_substitution(vec![])),
    };
    let self_type_name = match &impl_item.self_ty {
        glyim_hir::TypeRef::Path(p) => p.as_name(),
        _ => None,
    };
    let generic_param_names = impl_item.generic_params.iter().map(|p| p.name).collect();
    ResolvedImplHeader {
        trait_def_id, trait_name, trait_substs, self_ty, self_type_name,
        generic_param_names, polarity: ImplPolarity::Positive, span,
    }
}

fn resolve_path_type(
    ctx: &mut TyCtxMut,
    infer: infer: infer: &mut InferenceTablemut InferenceTablemut InferenceTable,
    def_map: &glyim_def_map::CrateDefMap,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    path: &glyim_hir::Path,
    param_map: &HashMap<Name, Ty>,
    span: Span,
) -> Ty {
    // Convert to core::Path for resolver
    let core_segments = path.segments.iter().map(|seg| glyim_core::PathSegment { name: seg.name }).collect();
    let core_path = CorePath { segments: core_segments, kind: path.kind.clone() };
    let resolver = Resolver::new(def_map, def_map.root);
    let per_ns = resolver.resolve_path(&core_path);
    if let Some((def_id, _)) = per_ns.types {
        let adt_id = AdtId::from_raw(def_id.to_raw());
        let substs = ctx.intern_substitution(vec![]);
        return ctx.mk_ty(TyKind::Adt(adt_id, substs));
    }
    if let Some(name) = path.as_name() {
        if let Some(&ty) = param_map.get(&name) {
            return ty;
        }
        if let Some(ty) = resolve_primitive(ctx, name) {
            return ty;
        }
    }
    diagnostics.push(GlyimDiagnostic::type_error(span, format!("unresolved type path: {:?}", path)));
    Ty::ERROR
}

fn resolve_primitive(ctx: &mut TyCtxMut, name: Name) -> Option<Ty> {
    let s = ctx.name_str(name);
    Some(match s {
        "i8" => ctx.mk_ty(TyKind::Int(IntTy::I8)),
        "i16" => ctx.mk_ty(TyKind::Int(IntTy::I16)),
        "i32" => ctx.mk_ty(TyKind::Int(IntTy::I32)),
        "i64" => ctx.mk_ty(TyKind::Int(IntTy::I64)),
        "isize" => ctx.mk_ty(TyKind::Int(IntTy::Isize)),
        "u8" => ctx.mk_ty(TyKind::Uint(UintTy::U8)),
        "u16" => ctx.mk_ty(TyKind::Uint(UintTy::U16)),
        "u32" => ctx.mk_ty(TyKind::Uint(UintTy::U32)),
        "u64" => ctx.mk_ty(TyKind::Uint(UintTy::U64)),
        "usize" => ctx.mk_ty(TyKind::Uint(UintTy::Usize)),
        "f32" => ctx.mk_ty(TyKind::Float(FloatTy::F32)),
        "f64" => ctx.mk_ty(TyKind::Float(FloatTy::F64)),
        "bool" => Ty::BOOL,
        "char" => ctx.mk_ty(TyKind::Char),
        "str" => ctx.mk_ty(TyKind::String),
        _ => return None,
    })
}

fn resolve_name_to_def_id(def_map: &glyim_def_map::CrateDefMap, name: Name) -> Option<DefId> {
    let res = def_map.modules[def_map.root].scope.resolve(name)?;
    Some(DefId::new(def_map.krate, res.0))
}
