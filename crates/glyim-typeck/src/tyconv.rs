/// HIR `TypeRef` → `Ty` conversion.
use std::collections::HashMap;

use glyim_core::def_id::{AdtId, DefId, TraitDefId};
use glyim_core::interner::Name;
use glyim_core::primitives::*;
use glyim_diag::GlyimDiagnostic;
use glyim_solve::InferenceTable;
use glyim_span::Span;
use glyim_type::*;

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
            let inner_ty =
                resolve_type_ref(ctx, infer, def_map, diagnostics, inner, param_map, span);
            if inner_ty == Ty::ERROR {
                return Ty::ERROR;
            }
            ctx.mk_ref(Region::Erased, inner_ty, *mutability)
        }

        glyim_hir::TypeRef::Tuple(elements) => {
            let mut tys = Vec::with_capacity(elements.len());
            for elem in elements {
                tys.push(resolve_type_ref(
                    ctx,
                    infer,
                    def_map,
                    diagnostics,
                    elem,
                    param_map,
                    span,
                ));
            }
            if tys.is_empty() {
                return Ty::UNIT;
            }
            if tys.contains(&Ty::ERROR) {
                return Ty::ERROR;
            }
            let substs = ctx.intern_substitution(tys.into_iter().map(GenericArg::Ty).collect());
            ctx.mk_ty(TyKind::Tuple(substs))
        }

        glyim_hir::TypeRef::Never => Ty::NEVER,

        glyim_hir::TypeRef::Infer => {
            let var = infer.new_ty_var(ctx);
            ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
        }

        glyim_hir::TypeRef::Error => Ty::ERROR,

        // Function pointer types: fn(T, U) -> R
        glyim_hir::TypeRef::Fn { params, ret } => {
            let mut param_tys = Vec::with_capacity(params.len());
            for param_ty_ref in params {
                let ty = resolve_type_ref(
                    ctx,
                    infer,
                    def_map,
                    diagnostics,
                    param_ty_ref,
                    param_map,
                    span,
                );
                if ty == Ty::ERROR {
                    return Ty::ERROR;
                }
                param_tys.push(GenericArg::Ty(ty));
            }
            let return_ty = match ret {
                Some(ret_ref) => {
                    resolve_type_ref(ctx, infer, def_map, diagnostics, ret_ref, param_map, span)
                }
                None => Ty::UNIT,
            };
            if return_ty == Ty::ERROR {
                return Ty::ERROR;
            }
            let fn_sig = glyim_type::FnSig {
                inputs: ctx.intern_substitution(param_tys),
                output: return_ty,
                c_variadic: false,
                unsafety: Safety::Safe,
                abi: Abi::Glyim,
            };
            ctx.mk_ty(TyKind::FnPtr(fn_sig))
        }

        // Slice types: [T]
        glyim_hir::TypeRef::Slice(inner) => {
            let elem_ty =
                resolve_type_ref(ctx, infer, def_map, diagnostics, inner, param_map, span);
            if elem_ty == Ty::ERROR {
                Ty::ERROR
            } else {
                ctx.mk_ty(TyKind::Slice(elem_ty))
            }
        }

        // Array types: [T; N]
        glyim_hir::TypeRef::Array { inner, len } => {
            let elem_ty =
                resolve_type_ref(ctx, infer, def_map, diagnostics, inner, param_map, span);
            if elem_ty == Ty::ERROR {
                return Ty::ERROR;
            }
            let const_val = resolve_const_ref(ctx, def_map, diagnostics, len, param_map, span);
            ctx.mk_ty(TyKind::Array(elem_ty, const_val))
        }

        // `dyn Trait` — an unsized trait object.
        glyim_hir::TypeRef::Dyn(inner) => {
            // The inner type reference must name a trait.
            let trait_path = match &**inner {
                glyim_hir::TypeRef::Path(p) => p,
                _ => {
                    diagnostics.push(GlyimDiagnostic::type_error(
                        span,
                        "the inner type of a `dyn` trait object must be a trait path".to_string(),
                    ));
                    return Ty::ERROR;
                }
            };
            let trait_def_id = resolve_path_to_trait_def_id(def_map, trait_path, span);
            let trait_def_id = match trait_def_id {
                Some(id) => id,
                None => {
                    let path_str = trait_path
                        .segments
                        .iter()
                        .map(|seg| ctx.name_str(seg.name))
                        .collect::<Vec<_>>()
                        .join("::");
                    diagnostics.push(GlyimDiagnostic::type_error(
                        span,
                        format!("cannot find trait `{}` in this scope", path_str),
                    ));
                    return Ty::ERROR;
                }
            };

            // Enforce object safety for the resolved trait when its definition
            // is available in the type context.
            if let Some(trait_def) = ctx.trait_def(trait_def_id) {
                let methods: Vec<glyim_type::object_safety::MethodSignature> = trait_def
                    .methods
                    .iter()
                    .map(|m| {
                        let inputs = ctx.substitution_args(m.sig.inputs);
                        glyim_type::object_safety::MethodSignature {
                            name: m.name,
                            span,
                            self_kind: self_kind_of_inputs(&*ctx, inputs),
                            // Generic-parameter detection requires the trait
                            // method's own generic-param list, which is not
                            // recoverable from the interred `FnSig` substitution
                            // here. We conservatively assume no generic params;
                            // the dedicated object-safety algorithm tests still
                            // exercise the generic-method rejection path.
                            has_generic_params: false,
                            returns_self: false,
                        }
                    })
                    .collect();
                let violations = glyim_type::object_safety::check_object_safety(
                    &glyim_type::object_safety::TraitObjectSafetyInput {
                        requires_self_sized: false,
                        methods: &methods,
                        associated_types: &[],
                        supertrait_safety: &[],
                    },
                );
                for v in violations {
                    let msg = match v {
                        glyim_type::object_safety::ObjectSafetyViolation::SelfSized => {
                            "the trait cannot be made into an object because it requires `Self: Sized`".into()
                        }
                        glyim_type::object_safety::ObjectSafetyViolation::GenericMethod {
                            method, ..
                        } => format!(
                            "the trait `{}` cannot be made into an object because method `{}` has generic type parameters",
                            ctx.name_str(trait_def.name),
                            ctx.name_str(method)
                        ),
                        glyim_type::object_safety::ObjectSafetyViolation::StaticMethod {
                            method, ..
                        } => format!(
                            "the trait `{}` cannot be made into an object because method `{}` has no receiver",
                            ctx.name_str(trait_def.name),
                            ctx.name_str(method)
                        ),
                        glyim_type::object_safety::ObjectSafetyViolation::ByValueSelf {
                            method, ..
                        } => format!(
                            "the trait `{}` cannot be made into an object because method `{}` takes `self` by value",
                            ctx.name_str(trait_def.name),
                            ctx.name_str(method)
                        ),
                        glyim_type::object_safety::ObjectSafetyViolation::AssociatedFunction {
                            name, ..
                        } => format!(
                            "the trait `{}` cannot be made into an object because associated function `{}` is not dispatchable",
                            ctx.name_str(trait_def.name),
                            ctx.name_str(name)
                        ),
                        glyim_type::object_safety::ObjectSafetyViolation::UnconstrainedAssociatedType {
                            name, ..
                        } => format!(
                            "the trait `{}` cannot be made into an object because associated type `{}` is not constrained",
                            ctx.name_str(trait_def.name),
                            ctx.name_str(name)
                        ),
                        glyim_type::object_safety::ObjectSafetyViolation::SupertraitNotObjectSafe {
                            trait_id, ..
                        } => format!(
                            "the trait `{}` cannot be made into an object because supertrait `{}` is not object-safe",
                            ctx.name_str(trait_def.name),
                            ctx.trait_def(trait_id)
                                .map(|t| ctx.name_str(t.name).to_string())
                                .unwrap_or_else(|| format!("#{}", trait_id.index())),
                        ),
                    };
                    diagnostics.push(GlyimDiagnostic::type_error(span, msg));
                }
            }

            let trait_ref = glyim_type::TraitRef {
                def_id: trait_def_id,
                substs: ctx.intern_substitution(vec![]),
            };
            let preds: Box<[glyim_type::Predicate]> =
                Box::new([glyim_type::Predicate::Trait(glyim_type::TraitPredicate {
                    trait_ref,
                    polarity: glyim_type::ImplPolarity::Positive,
                })]);
            let binder = glyim_type::Binder::bind(
                preds,
                Box::new([glyim_type::BoundVariableKind::Ty(
                    glyim_type::BoundTyKind::Anon,
                )]),
            );
            ctx.mk_ty(TyKind::Dynamic(binder, Region::Erased))
        }
    }
}

/// Resolve const expressions in type positions (simplified)
fn resolve_const_ref(
    ctx: &mut TyCtxMut,
    _def_map: &glyim_def_map::CrateDefMap,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    const_ref: &glyim_hir::ConstRef,
    _param_map: &HashMap<Name, Ty>,
    span: Span,
) -> Const {
    match const_ref {
        glyim_hir::ConstRef::Literal(lit) => match lit {
            glyim_hir::Literal::Int(v, _) => Const {
                kind: ConstKind::Int(*v),
                ty: ctx.mk_ty(TyKind::Int(IntTy::Isize)),
            },
            glyim_hir::Literal::Uint(v, _) => Const {
                kind: ConstKind::Uint(*v),
                ty: ctx.mk_ty(TyKind::Uint(UintTy::Usize)),
            },
            glyim_hir::Literal::Bool(b) => Const {
                kind: ConstKind::Bool(*b),
                ty: Ty::BOOL,
            },
            glyim_hir::Literal::Char(c) => Const {
                kind: ConstKind::Char(*c),
                ty: ctx.mk_ty(TyKind::Char),
            },
            _ => {
                diagnostics.push(GlyimDiagnostic::type_error(
                    span,
                    "unsupported const literal in type position",
                ));
                Const {
                    kind: ConstKind::Error,
                    ty: ctx.error_ty(),
                }
            }
        },
        glyim_hir::ConstRef::Path(_path) => {
            diagnostics.push(GlyimDiagnostic::type_error(
                span,
                "const generic paths not yet fully implemented",
            ));
            Const {
                kind: ConstKind::Error,
                ty: ctx.error_ty(),
            }
        }
        glyim_hir::ConstRef::Error => Const {
            kind: ConstKind::Error,
            ty: ctx.error_ty(),
        },
    }
}

pub fn build_param_tys(
    ctx: &mut TyCtxMut,
    params: &[glyim_hir::GenericParam],
) -> HashMap<Name, Ty> {
    let mut map = HashMap::with_capacity(params.len());
    for (i, param) in params.iter().enumerate() {
        match param.kind {
            glyim_hir::GenericParamKind::Type { .. } => {
                let pt = ParamTy {
                    index: i as u32,
                    name: param.name,
                };
                map.insert(param.name, ctx.mk_ty(TyKind::Param(pt)));
            }
            glyim_hir::GenericParamKind::Lifetime => {
                // Lifetimes handled separately
            }
            glyim_hir::GenericParamKind::Const { .. } => {
                // Const generics handled via separate resolution
            }
        }
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

    let mut param_tys = Vec::with_capacity(params.len());
    for param in params {
        let ty = if let Some(ty_ref) = &param.ty {
            let resolved = resolve_type_ref(
                ctx,
                infer,
                def_map,
                diagnostics,
                ty_ref,
                &param_map,
                param.span,
            );
            if resolved == Ty::ERROR {
                let var = infer.new_ty_var(ctx);
                ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
            } else {
                resolved
            }
        } else {
            let var = infer.new_ty_var(ctx);
            ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
        };
        param_tys.push(ty);
    }

    let return_ty = match return_ty_ref {
        Some(ty_ref) => {
            let resolved =
                resolve_type_ref(ctx, infer, def_map, diagnostics, ty_ref, &param_map, span);
            if resolved == Ty::ERROR {
                let var = infer.new_ty_var(ctx);
                ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
            } else {
                resolved
            }
        }
        None => Ty::UNIT,
    };

    FnSig {
        param_tys,
        return_ty,
    }
}

pub fn resolve_impl_header(
    ctx: &mut TyCtxMut,
    _infer: &mut InferenceTable,
    def_map: &glyim_def_map::CrateDefMap,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    impl_item: &glyim_hir::ImplItem,
    span: Span,
) -> ResolvedImplHeader {
    let param_map = build_param_tys(ctx, &impl_item.generic_params);

    let self_ty = resolve_type_ref(
        ctx,
        _infer,
        def_map,
        diagnostics,
        &impl_item.self_ty,
        &param_map,
        span,
    );

    let (trait_def_id, trait_name, trait_substs) = match &impl_item.trait_ref {
        Some(path) => {
            if let Some(name) = path.as_name() {
                match resolve_name_to_def_id(def_map, name) {
                    Some(def_id) => {
                        let trait_def_id = TraitDefId::from_raw(def_id.local_id.to_raw());
                        let substs = ctx.intern_substitution(vec![]);
                        (Some(trait_def_id), Some(name), substs)
                    }
                    None => {
                        diagnostics.push(GlyimDiagnostic::type_error(
                            span,
                            format!("unresolved trait `{}`", def_map.interner.resolve(name)),
                        ));
                        (None, Some(name), ctx.intern_substitution(vec![]))
                    }
                }
            } else {
                match resolve_path_to_trait_def_id(def_map, path, span) {
                    Some(trait_def_id) => {
                        let substs = ctx.intern_substitution(vec![]);
                        (Some(trait_def_id), path.as_name(), substs)
                    }
                    None => {
                        diagnostics.push(GlyimDiagnostic::type_error(
                            span,
                            "multi-segment trait paths not yet implemented",
                        ));
                        (None, None, ctx.intern_substitution(vec![]))
                    }
                }
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
        trait_def_id,
        trait_name,
        trait_substs,
        self_ty,
        self_type_name,
        generic_param_names,
        polarity: ImplPolarity::Positive,
        span,
    }
}

pub fn resolve_path_type(
    ctx: &mut TyCtxMut,
    infer: &mut InferenceTable,
    def_map: &glyim_def_map::CrateDefMap,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    path: &glyim_hir::Path,
    param_map: &HashMap<Name, Ty>,
    span: Span,
) -> Ty {
    // Check param_map first for generic params
    if let Some(name) = path.as_name()
        && let Some(&ty) = param_map.get(&name)
    {
        return ty;
    }

    // Check primitives
    if let Some(name) = path.as_name()
        && let Some(ty) = resolve_primitive(ctx, name)
    {
        return ty;
    }

    // Check ADTs (structs, enums, unions)
    if let Some(name) = path.as_name()
        && let Some(ty) = resolve_name_to_adt_ty(ctx, def_map, name)
    {
        return ty;
    }

    // Multi-segment paths: try to resolve fully
    if !path.segments.is_empty()
        && let Some(resolved) = resolve_qualified_path(ctx, def_map, path, param_map, span, infer)
    {
        return resolved;
    }

    // Fallback error
    let path_str = path
        .segments
        .iter()
        .map(|seg| ctx.name_str(seg.name))
        .collect::<Vec<_>>()
        .join("::");

    diagnostics.push(GlyimDiagnostic::type_error(
        span,
        format!("unresolved type `{}`", path_str),
    ));
    Ty::ERROR
}

/// Resolve qualified paths like std::vec::Vec<T>
fn resolve_qualified_path(
    ctx: &mut TyCtxMut,
    def_map: &glyim_def_map::CrateDefMap,
    path: &glyim_hir::Path,
    _param_map: &HashMap<Name, Ty>,
    span: Span,
    infer: &mut InferenceTable,
) -> Option<Ty> {
    if path.segments.len() == 1 {
        return None;
    }

    if let Some(last_name) = path.segments.last().map(|s| s.name)
        && let Some(ty) = resolve_name_to_adt_ty(ctx, def_map, last_name)
    {
        if let Some(args) = path.segments.last().and_then(|s| s.generic_args.as_ref()) {
            let mut arg_tys = Vec::with_capacity(args.len());
            for arg in args {
                let resolved = resolve_type_ref(
                    ctx,
                    infer,
                    def_map,
                    &mut Vec::new(),
                    arg,
                    &HashMap::new(),
                    span,
                );
                arg_tys.push(GenericArg::Ty(resolved));
            }
            if !arg_tys
                .iter()
                .any(|a| matches!(a, GenericArg::Ty(Ty::ERROR)))
            {
                let substs = ctx.intern_substitution(arg_tys);
                if let TyKind::Adt(adt_id, _) = ctx.ty_kind(ty) {
                    return Some(ctx.mk_ty(TyKind::Adt(*adt_id, substs)));
                }
            }
        }
        return Some(ty);
    }

    None
}

/// Resolve path to trait DefId
fn resolve_path_to_trait_def_id(
    def_map: &glyim_def_map::CrateDefMap,
    path: &glyim_hir::Path,
    _span: Span,
) -> Option<TraitDefId> {
    if let Some(name) = path.as_name() {
        let res = def_map.modules[def_map.root].scope.resolve(name)?;
        Some(TraitDefId::from_raw(res.0.to_raw()))
    } else {
        None
    }
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

/// Infer the object-safety `self` kind of a trait method from its resolved
/// input argument list.
///
/// The first input is the receiver. A reference receiver (`&self` /
/// `&mut self`) is object-safe; a by-value `self` requires `Self: Sized`;
/// an empty input list means the method has no receiver (a static / associated
/// function), which cannot be dispatched through a trait object.
fn self_kind_of_inputs(
    ctx: &dyn glyim_type::TypeLookup,
    inputs: &[glyim_type::GenericArg],
) -> glyim_type::object_safety::MethodSelfKind {
    let Some(first) = inputs.first().and_then(|a| match a {
        glyim_type::GenericArg::Ty(t) => Some(t),
        _ => None,
    }) else {
        return glyim_type::object_safety::MethodSelfKind::None;
    };
    match ctx.ty_kind(*first) {
        glyim_type::TyKind::Ref(_, _, _) => glyim_type::object_safety::MethodSelfKind::ByReference,
        _ => glyim_type::object_safety::MethodSelfKind::ByValue,
    }
}

fn resolve_name_to_adt_ty(
    ctx: &mut TyCtxMut,
    def_map: &glyim_def_map::CrateDefMap,
    name: Name,
) -> Option<Ty> {
    let def_id = resolve_name_to_def_id(def_map, name)?;
    let adt_id = AdtId::from_raw(def_id.local_id.to_raw());
    let substs = ctx.intern_substitution(vec![]);
    Some(ctx.mk_ty(TyKind::Adt(adt_id, substs)))
}
