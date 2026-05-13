use glyim_core::arena::IndexVec;
use glyim_diag::GlyimDiagnostic;
use glyim_span::Span;
use glyim_type::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VariableKind {
    General,
    Integer,
    Float,
}

#[derive(Clone, Debug)]
pub struct TypeVariable {
    pub universe: UniverseIndex,
    pub value: Option<Ty>,
    pub kind: VariableKind,
}

#[derive(Clone, Debug)]
pub struct RegionVariable {
    pub universe: UniverseIndex,
    pub value: Option<Region>,
}

/// [F18] InferenceTable — separate IndexVecs
pub struct InferenceTable {
    ty_vars: IndexVec<TyVar, TypeVariable>,
    int_vars: IndexVec<IntVar, TypeVariable>,
    float_vars: IndexVec<FloatVar, TypeVariable>,
    region_vars: IndexVec<RegionVid, RegionVariable>,
    universe: UniverseIndex,
}

impl InferenceTable {
    pub fn new() -> Self {
        Self {
            ty_vars: IndexVec::new(),
            int_vars: IndexVec::new(),
            float_vars: IndexVec::new(),
            region_vars: IndexVec::new(),
            universe: UniverseIndex(0),
        }
    }

    pub fn new_ty_var(&mut self, _ctx: &mut TyCtxMut) -> TyVar {
        self.ty_vars.push(TypeVariable {
            universe: self.universe,
            value: None,
            kind: VariableKind::General,
        })
    }

    pub fn new_int_var(&mut self, _ctx: &mut TyCtxMut) -> IntVar {
        self.int_vars.push(TypeVariable {
            universe: self.universe,
            value: None,
            kind: VariableKind::Integer,
        })
    }

    pub fn new_float_var(&mut self, _ctx: &mut TyCtxMut) -> FloatVar {
        self.float_vars.push(TypeVariable {
            universe: self.universe,
            value: None,
            kind: VariableKind::Float,
        })
    }

    pub fn new_region_var(&mut self, _ctx: &mut TyCtxMut) -> RegionVid {
        self.region_vars.push(RegionVariable {
            universe: self.universe,
            value: None,
        })
    }

    pub fn universe(&self) -> UniverseIndex {
        self.universe
    }
    pub fn create_universe(&mut self) -> UniverseIndex {
        self.universe = UniverseIndex(self.universe.0 + 1);
        self.universe
    }

    pub fn probe_ty_var(&self, var: TyVar) -> Option<Ty> {
        self.ty_vars.get(var).and_then(|v| v.value)
    }

    pub fn probe_int_var(&self, var: IntVar) -> Option<Ty> {
        self.int_vars.get(var).and_then(|v| v.value)
    }

    pub fn probe_float_var(&self, var: FloatVar) -> Option<Ty> {
        self.float_vars.get(var).and_then(|v| v.value)
    }

    pub fn unify(
        &mut self,
        ctx: &mut TyCtxMut,
        a: Ty,
        b: Ty,
        span: Span,
    ) -> Result<Vec<Constraint>, Vec<GlyimDiagnostic>> {
        let a = self.resolve_ty_shallow(ctx, a);
        let b = self.resolve_ty_shallow(ctx, b);
        self.unify_tys(ctx, a, b, span)
    }

    fn unify_tys(
        &mut self,
        ctx: &mut TyCtxMut,
        a: Ty,
        b: Ty,
        span: Span,
    ) -> Result<Vec<Constraint>, Vec<GlyimDiagnostic>> {
        if a == b {
            return Ok(Vec::new());
        }
        let a_kind = ctx.ty_kind(a).clone();
        let b_kind = ctx.ty_kind(b).clone();

        let a_is_int = matches!(a_kind, TyKind::Infer(InferVar::Int(_)));
        let a_is_float = matches!(a_kind, TyKind::Infer(InferVar::Float(_)));
        match (a_kind, b_kind) {
            (TyKind::Error, _) | (_, TyKind::Error) => Ok(Vec::new()),
            (TyKind::Never, _) | (_, TyKind::Never) => Ok(Vec::new()),
            (TyKind::Infer(InferVar::Int(var)), other)
            | (other, TyKind::Infer(InferVar::Int(var))) => {
                let int_ty = if a_is_int { a } else { b };
                match &other {
                    TyKind::Int(_) | TyKind::Infer(InferVar::Int(_)) | TyKind::Error => {
                        self.int_vars[var].value = Some(b);
                        Ok(Vec::new())
                    }
                    TyKind::Infer(InferVar::Ty(general)) => {
                        self.ty_vars[*general].value = Some(int_ty);
                        Ok(Vec::new())
                    }
                    _ => Err(vec![GlyimDiagnostic::type_error(
                        span,
                        format!(
                            "mismatched types: expected integer type, found {}",
                            PrintTy::new(b, ctx)
                        ),
                    )]),
                }
            }
            (TyKind::Infer(InferVar::Float(var)), other)
            | (other, TyKind::Infer(InferVar::Float(var))) => {
                let float_ty = if a_is_float { a } else { b };
                match &other {
                    TyKind::Float(_) | TyKind::Infer(InferVar::Float(_)) | TyKind::Error => {
                        self.float_vars[var].value = Some(b);
                        Ok(Vec::new())
                    }
                    TyKind::Infer(InferVar::Ty(general)) => {
                        self.ty_vars[*general].value = Some(float_ty);
                        Ok(Vec::new())
                    }
                    _ => Err(vec![GlyimDiagnostic::type_error(
                        span,
                        format!(
                            "mismatched types: expected float type, found {}",
                            PrintTy::new(b, ctx)
                        ),
                    )]),
                }
            }
            (TyKind::Infer(InferVar::Ty(var)), other)
            | (other, TyKind::Infer(InferVar::Ty(var))) => {
                let tv = &self.ty_vars[var];
                match tv.kind {
                    VariableKind::General => {
                        self.ty_vars[var].value = Some(b);
                        Ok(Vec::new())
                    }
                    VariableKind::Integer => match &other {
                        TyKind::Int(_) | TyKind::Error => {
                            self.ty_vars[var].value = Some(b);
                            Ok(Vec::new())
                        }
                        TyKind::Infer(InferVar::Int(_)) | TyKind::Infer(InferVar::Ty(_)) => {
                            self.ty_vars[var].value = Some(b);
                            Ok(Vec::new())
                        }
                        _ => Err(vec![GlyimDiagnostic::type_error(
                            span,
                            format!(
                                "mismatched types: expected integer type, found {}",
                                PrintTy::new(b, ctx)
                            ),
                        )]),
                    },
                    VariableKind::Float => match &other {
                        TyKind::Float(_) | TyKind::Error => {
                            self.ty_vars[var].value = Some(b);
                            Ok(Vec::new())
                        }
                        TyKind::Infer(InferVar::Float(_)) | TyKind::Infer(InferVar::Ty(_)) => {
                            self.ty_vars[var].value = Some(b);
                            Ok(Vec::new())
                        }
                        _ => Err(vec![GlyimDiagnostic::type_error(
                            span,
                            format!(
                                "mismatched types: expected float type, found {}",
                                PrintTy::new(b, ctx)
                            ),
                        )]),
                    },
                }
            }
            (TyKind::Ref(r_a, ty_a, mut_a), TyKind::Ref(r_b, ty_b, mut_b)) => {
                if mut_a != mut_b {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        format!("mismatched mutability: {:?} vs {:?}", mut_a, mut_b),
                    )]);
                }
                let mut constraints = vec![Constraint::RegionEq { a: r_a, b: r_b }];
                constraints.extend(self.unify_tys(ctx, ty_a, ty_b, span)?);
                Ok(constraints)
            }
            (TyKind::Int(int_a), TyKind::Int(int_b)) if int_a == int_b => Ok(Vec::new()),
            (TyKind::Uint(uint_a), TyKind::Uint(uint_b)) if uint_a == uint_b => Ok(Vec::new()),
            (TyKind::Float(float_a), TyKind::Float(float_b)) if float_a == float_b => {
                Ok(Vec::new())
            }
            (TyKind::Bool, TyKind::Bool) => Ok(Vec::new()),
            (TyKind::Char, TyKind::Char) => Ok(Vec::new()),
            (TyKind::String, TyKind::String) => Ok(Vec::new()),
            (TyKind::Unit, TyKind::Unit) => Ok(Vec::new()),
            (TyKind::Tuple(substs_a), TyKind::Tuple(substs_b)) => {
                let args_a = ctx.substitution_args(substs_a);
                let args_b = ctx.substitution_args(substs_b);
                if args_a.len() != args_b.len() {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        format!(
                            "mismatched types: {} vs {}",
                            PrintTy::new(a, ctx),
                            PrintTy::new(b, ctx)
                        ),
                    )]);
                }
                let pairs: Vec<(Ty, Ty)> = args_a
                    .iter()
                    .zip(args_b.iter())
                    .filter_map(|(ga, gb)| match (ga, gb) {
                        (GenericArg::Ty(ta), GenericArg::Ty(tb)) => Some((*ta, *tb)),
                        _ => None,
                    })
                    .collect();
                if pairs.len() != args_a.len() {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched generic arguments in tuple".to_string(),
                    )]);
                }
                let mut constraints = Vec::new();
                for (ta, tb) in pairs {
                    constraints.extend(self.unify_tys(ctx, ta, tb, span)?);
                }
                Ok(constraints)
            }
            (TyKind::Array(elem_a, const_a), TyKind::Array(elem_b, const_b)) => {
                if const_a.kind != const_b.kind || const_a.ty != const_b.ty {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched array lengths".to_string(),
                    )]);
                }
                self.unify_tys(ctx, elem_a, elem_b, span)
            }
            (TyKind::Slice(elem_a), TyKind::Slice(elem_b)) => {
                self.unify_tys(ctx, elem_a, elem_b, span)
            }
            (TyKind::RawPtr(inner_a, mut_a), TyKind::RawPtr(inner_b, mut_b)) => {
                if mut_a != mut_b {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        format!("mismatched mutability: {:?} vs {:?}", mut_a, mut_b),
                    )]);
                }
                self.unify_tys(ctx, inner_a, inner_b, span)
            }
            (TyKind::FnPtr(sig_a), TyKind::FnPtr(sig_b)) => {
                if sig_a.unsafety != sig_b.unsafety
                    || sig_a.abi != sig_b.abi
                    || sig_a.c_variadic != sig_b.c_variadic
                {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched function signatures".to_string(),
                    )]);
                }
                let inputs_a = ctx.substitution_args(sig_a.inputs);
                let inputs_b = ctx.substitution_args(sig_b.inputs);
                if inputs_a.len() != inputs_b.len() {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched function argument counts".to_string(),
                    )]);
                }
                let pairs: Vec<(Ty, Ty)> = inputs_a
                    .iter()
                    .zip(inputs_b.iter())
                    .filter_map(|(ga, gb)| match (ga, gb) {
                        (GenericArg::Ty(ta), GenericArg::Ty(tb)) => Some((*ta, *tb)),
                        _ => None,
                    })
                    .collect();
                if pairs.len() != inputs_a.len() {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched function argument kinds".to_string(),
                    )]);
                }
                let mut constraints = Vec::new();
                for (ta, tb) in pairs {
                    constraints.extend(self.unify_tys(ctx, ta, tb, span)?);
                }
                constraints.extend(self.unify_tys(ctx, sig_a.output, sig_b.output, span)?);
                Ok(constraints)
            }
            (TyKind::Adt(id_a, substs_a), TyKind::Adt(id_b, substs_b)) => {
                if id_a != id_b {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        format!(
                            "mismatched types: {} vs {}",
                            PrintTy::new(a, ctx),
                            PrintTy::new(b, ctx)
                        ),
                    )]);
                }
                let args_a = ctx.substitution_args(substs_a);
                let args_b = ctx.substitution_args(substs_b);
                if args_a.len() != args_b.len() {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched type argument counts".to_string(),
                    )]);
                }
                let pairs: Vec<(Ty, Ty)> = args_a
                    .iter()
                    .zip(args_b.iter())
                    .filter_map(|(ga, gb)| match (ga, gb) {
                        (GenericArg::Ty(ta), GenericArg::Ty(tb)) => Some((*ta, *tb)),
                        _ => None,
                    })
                    .collect();
                if pairs.len() != args_a.len() {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched generic argument kinds in Adt".to_string(),
                    )]);
                }
                let mut constraints = Vec::new();
                for (ta, tb) in pairs {
                    constraints.extend(self.unify_tys(ctx, ta, tb, span)?);
                }
                Ok(constraints)
            }
            (TyKind::FnDef(id_a, substs_a), TyKind::FnDef(id_b, substs_b)) => {
                if id_a != id_b {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        format!(
                            "mismatched types: {} vs {}",
                            PrintTy::new(a, ctx),
                            PrintTy::new(b, ctx)
                        ),
                    )]);
                }
                let args_a = ctx.substitution_args(substs_a);
                let args_b = ctx.substitution_args(substs_b);
                if args_a.len() != args_b.len() {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched type argument counts".to_string(),
                    )]);
                }
                let pairs: Vec<(Ty, Ty)> = args_a
                    .iter()
                    .zip(args_b.iter())
                    .filter_map(|(ga, gb)| match (ga, gb) {
                        (GenericArg::Ty(ta), GenericArg::Ty(tb)) => Some((*ta, *tb)),
                        _ => None,
                    })
                    .collect();
                if pairs.len() != args_a.len() {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched generic argument kinds in FnDef".to_string(),
                    )]);
                }
                let mut constraints = Vec::new();
                for (ta, tb) in pairs {
                    constraints.extend(self.unify_tys(ctx, ta, tb, span)?);
                }
                Ok(constraints)
            }
            (TyKind::Closure(id_a, substs_a), TyKind::Closure(id_b, substs_b)) => {
                if id_a != id_b {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        format!(
                            "mismatched types: {} vs {}",
                            PrintTy::new(a, ctx),
                            PrintTy::new(b, ctx)
                        ),
                    )]);
                }
                let args_a = ctx.substitution_args(substs_a);
                let args_b = ctx.substitution_args(substs_b);
                if args_a.len() != args_b.len() {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched type argument counts".to_string(),
                    )]);
                }
                let pairs: Vec<(Ty, Ty)> = args_a
                    .iter()
                    .zip(args_b.iter())
                    .filter_map(|(ga, gb)| match (ga, gb) {
                        (GenericArg::Ty(ta), GenericArg::Ty(tb)) => Some((*ta, *tb)),
                        _ => None,
                    })
                    .collect();
                if pairs.len() != args_a.len() {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched generic argument kinds in Closure".to_string(),
                    )]);
                }
                let mut constraints = Vec::new();
                for (ta, tb) in pairs {
                    constraints.extend(self.unify_tys(ctx, ta, tb, span)?);
                }
                Ok(constraints)
            }
            (TyKind::Dynamic(_, r_a), TyKind::Dynamic(_, r_b)) if r_a == r_b => {
                // Predicate unification is NYI; treat as compatible for now
                tracing::warn!("STUB: Dynamic predicate unification not implemented");
                Ok(Vec::new())
            }
            (TyKind::Opaque(id_a, substs_a), TyKind::Opaque(id_b, substs_b)) => {
                if id_a != id_b {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        format!(
                            "mismatched types: {} vs {}",
                            PrintTy::new(a, ctx),
                            PrintTy::new(b, ctx)
                        ),
                    )]);
                }
                let args_a = ctx.substitution_args(substs_a);
                let args_b = ctx.substitution_args(substs_b);
                if args_a.len() != args_b.len() {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched type argument counts".to_string(),
                    )]);
                }
                let pairs: Vec<(Ty, Ty)> = args_a
                    .iter()
                    .zip(args_b.iter())
                    .filter_map(|(ga, gb)| match (ga, gb) {
                        (GenericArg::Ty(ta), GenericArg::Ty(tb)) => Some((*ta, *tb)),
                        _ => None,
                    })
                    .collect();
                if pairs.len() != args_a.len() {
                    return Err(vec![GlyimDiagnostic::type_error(
                        span,
                        "mismatched generic argument kinds in Opaque".to_string(),
                    )]);
                }
                let mut constraints = Vec::new();
                for (ta, tb) in pairs {
                    constraints.extend(self.unify_tys(ctx, ta, tb, span)?);
                }
                Ok(constraints)
            }
            (_a_k, _b_k) => Err(vec![GlyimDiagnostic::type_error(
                span,
                format!(
                    "mismatched types: {} vs {}",
                    PrintTy::new(a, ctx),
                    PrintTy::new(b, ctx)
                ),
            )]),
        }
    }

    pub fn resolve_ty_shallow(&self, ctx: &dyn TypeLookup, ty: Ty) -> Ty {
        self.resolve_ty_shallow_depth(ctx, ty, 0)
    }

    fn resolve_ty_shallow_depth(&self, ctx: &dyn TypeLookup, ty: Ty, depth: u32) -> Ty {
        if depth > 256 {
            tracing::warn!("STUB: resolve_ty_shallow exceeded depth limit; possible cycle");
            return ty;
        }
        match ctx.ty_kind(ty) {
            TyKind::Infer(InferVar::Ty(var)) => {
                if let Some(value) = self.ty_vars.get(*var).and_then(|v| v.value) {
                    return self.resolve_ty_shallow_depth(ctx, value, depth + 1);
                }
                ty
            }
            TyKind::Infer(InferVar::Int(var)) => {
                if let Some(value) = self.int_vars.get(*var).and_then(|v| v.value) {
                    return self.resolve_ty_shallow_depth(ctx, value, depth + 1);
                }
                ty
            }
            TyKind::Infer(InferVar::Float(var)) => {
                if let Some(value) = self.float_vars.get(*var).and_then(|v| v.value) {
                    return self.resolve_ty_shallow_depth(ctx, value, depth + 1);
                }
                ty
            }
            _ => ty,
        }
    }

    pub fn fully_resolve(&self, ctx: &dyn TypeLookup, ty: Ty) -> Result<Ty, Vec<TyVar>> {
        let resolved = self.resolve_ty_shallow(ctx, ty);
        if self.has_unresolved_non_ty_infer(ctx, resolved) {
            return Err(Vec::new());
        }
        if ctx.ty_flags(resolved).contains(TypeFlags::HAS_TY_INFER) {
            let mut unresolved = Vec::new();
            self.collect_unresolved_vars(ctx, resolved, &mut unresolved);
            if unresolved.is_empty() {
                Ok(resolved)
            } else {
                Err(unresolved)
            }
        } else {
            Ok(resolved)
        }
    }

    fn has_unresolved_non_ty_infer(&self, ctx: &dyn TypeLookup, ty: Ty) -> bool {
        match ctx.ty_kind(ty) {
            TyKind::Infer(InferVar::Int(var)) => {
                self.int_vars.get(*var).is_none_or(|v| v.value.is_none())
            }
            TyKind::Infer(InferVar::Float(var)) => {
                self.float_vars.get(*var).is_none_or(|v| v.value.is_none())
            }
            TyKind::Infer(InferVar::Ty(_)) => false,
            TyKind::Ref(_, inner, _) => self.has_unresolved_non_ty_infer(ctx, *inner),
            TyKind::RawPtr(inner, _) => self.has_unresolved_non_ty_infer(ctx, *inner),
            TyKind::Slice(inner) => self.has_unresolved_non_ty_infer(ctx, *inner),
            TyKind::Array(inner, _) => self.has_unresolved_non_ty_infer(ctx, *inner),
            TyKind::Adt(_, substs)
            | TyKind::FnDef(_, substs)
            | TyKind::Closure(_, substs)
            | TyKind::Opaque(_, substs)
            | TyKind::Tuple(substs) => {
                for arg in ctx.substitution_args(*substs) {
                    if let GenericArg::Ty(t) = arg
                        && self.has_unresolved_non_ty_infer(ctx, *t)
                    {
                        return true;
                    }
                }
                false
            }
            TyKind::FnPtr(sig) => {
                for arg in ctx.substitution_args(sig.inputs) {
                    if let GenericArg::Ty(t) = arg
                        && self.has_unresolved_non_ty_infer(ctx, *t)
                    {
                        return true;
                    }
                }
                self.has_unresolved_non_ty_infer(ctx, sig.output)
            }
            _ => false,
        }
    }

    fn collect_unresolved_vars(&self, ctx: &dyn TypeLookup, ty: Ty, vars: &mut Vec<TyVar>) {
        match ctx.ty_kind(ty) {
            TyKind::Infer(InferVar::Ty(var)) => {
                if let Some(tv) = self.ty_vars.get(*var) {
                    if tv.value.is_none() {
                        vars.push(*var);
                    } else if let Some(resolved) = tv.value {
                        self.collect_unresolved_vars(ctx, resolved, vars);
                    }
                }
            }
            TyKind::Infer(InferVar::Int(_)) | TyKind::Infer(InferVar::Float(_)) => {}
            TyKind::Ref(_, inner, _) => self.collect_unresolved_vars(ctx, *inner, vars),
            TyKind::RawPtr(inner, _) => self.collect_unresolved_vars(ctx, *inner, vars),
            TyKind::Slice(inner) => self.collect_unresolved_vars(ctx, *inner, vars),
            TyKind::Array(inner, _) => self.collect_unresolved_vars(ctx, *inner, vars),
            TyKind::Adt(_, substs)
            | TyKind::FnDef(_, substs)
            | TyKind::Closure(_, substs)
            | TyKind::Opaque(_, substs)
            | TyKind::Tuple(substs) => {
                for arg in ctx.substitution_args(*substs) {
                    if let GenericArg::Ty(t) = arg {
                        self.collect_unresolved_vars(ctx, *t, vars);
                    }
                }
            }
            TyKind::FnPtr(sig) => {
                for arg in ctx.substitution_args(sig.inputs) {
                    if let GenericArg::Ty(t) = arg {
                        self.collect_unresolved_vars(ctx, *t, vars);
                    }
                }
                self.collect_unresolved_vars(ctx, sig.output, vars);
            }
            _ => {}
        }
    }

    // Test helpers
    #[cfg(test)]
    pub(crate) fn ty_var_kind(&self, var: TyVar) -> Option<VariableKind> {
        self.ty_vars.get(var).map(|tv| tv.kind)
    }
    #[cfg(test)]
    pub(crate) fn set_ty_var_value(&mut self, var: TyVar, value: Ty) {
        self.ty_vars[var].value = Some(value);
    }
    #[cfg(test)]
    pub(crate) fn set_int_var_value(&mut self, var: IntVar, value: Ty) {
        self.int_vars[var].value = Some(value);
    }
    #[cfg(test)]
    pub(crate) fn set_float_var_value(&mut self, var: FloatVar, value: Ty) {
        self.float_vars[var].value = Some(value);
    }
}

impl Default for InferenceTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub enum Constraint {
    TypeEq { a: Ty, b: Ty },
    RegionEq { a: Region, b: Region },
    RegionOutlives { a: Region, b: Region },
    TypeOutlives { ty: Ty, region: Region },
}
