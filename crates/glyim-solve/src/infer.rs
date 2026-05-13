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

        match (a_kind, b_kind) {
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
            (TyKind::Infer(InferVar::Int(var)), other)
            | (other, TyKind::Infer(InferVar::Int(var))) => match &other {
                TyKind::Int(_) | TyKind::Infer(InferVar::Int(_)) | TyKind::Error => {
                    self.int_vars[var].value = Some(b);
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
            (TyKind::Infer(InferVar::Float(var)), other)
            | (other, TyKind::Infer(InferVar::Float(var))) => match &other {
                TyKind::Float(_) | TyKind::Infer(InferVar::Float(_)) | TyKind::Error => {
                    self.float_vars[var].value = Some(b);
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
            (TyKind::Error, _) | (_, TyKind::Error) => Ok(Vec::new()),
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
        match ctx.ty_kind(ty) {
            TyKind::Infer(InferVar::Ty(var)) => {
                if let Some(value) = self.ty_vars.get(*var).and_then(|v| v.value) {
                    return self.resolve_ty_shallow(ctx, value);
                }
                ty
            }
            TyKind::Infer(InferVar::Int(var)) => {
                if let Some(value) = self.int_vars.get(*var).and_then(|v| v.value) {
                    return self.resolve_ty_shallow(ctx, value);
                }
                ty
            }
            TyKind::Infer(InferVar::Float(var)) => {
                if let Some(value) = self.float_vars.get(*var).and_then(|v| v.value) {
                    return self.resolve_ty_shallow(ctx, value);
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
            TyKind::Infer(InferVar::Int(var)) => self.int_vars.get(*var).map_or(true, |v| v.value.is_none()),
            TyKind::Infer(InferVar::Float(var)) => self.float_vars.get(*var).map_or(true, |v| v.value.is_none()),
            TyKind::Infer(InferVar::Ty(_)) => false,
            TyKind::Ref(_, inner, _) => self.has_unresolved_non_ty_infer(ctx, *inner),
            TyKind::RawPtr(inner, _) => self.has_unresolved_non_ty_infer(ctx, *inner),
            TyKind::Slice(inner) => self.has_unresolved_non_ty_infer(ctx, *inner),
            TyKind::Array(inner, _) => self.has_unresolved_non_ty_infer(ctx, *inner),
            TyKind::Adt(_, substs)
            | TyKind::FnDef(_, substs)
            | TyKind::Closure(_, substs)
            | TyKind::Tuple(substs) => {
                for arg in ctx.substitution_args(*substs) {
                    if let GenericArg::Ty(t) = arg {
                        if self.has_unresolved_non_ty_infer(ctx, *t) {
                            return true;
                        }
                    }
                }
                false
            }
            TyKind::FnPtr(sig) => {
                for arg in ctx.substitution_args(sig.inputs) {
                    if let GenericArg::Ty(t) = arg {
                        if self.has_unresolved_non_ty_infer(ctx, *t) {
                            return true;
                        }
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
