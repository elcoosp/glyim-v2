//! Coherence checker: orphan rules, overlap detection, and negative impls.

use std::collections::HashMap;

use glyim_core::def_id::TraitDefId;
use glyim_core::interner::Name;
use glyim_diag::{DiagSeverity, GlyimDiagnostic, SubDiagnostic};
use glyim_span::Span;
use glyim_type::{ImplPolarity, Substitution, Ty, TyCtxMut, TyKind};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ResolvedImplHeader {
    pub trait_def_id: Option<TraitDefId>,
    pub trait_name: Option<Name>,
    pub trait_substs: Substitution,
    pub self_ty: Ty,
    pub self_type_name: Option<Name>,
    pub generic_param_names: Vec<Name>,
    pub polarity: ImplPolarity,
    pub span: Span,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct RegisteredImpl {
    trait_def_id: TraitDefId,
    self_ty: Ty,
    self_type_name: Option<Name>,
    is_blanket: bool,
    polarity: ImplPolarity,
    span: Span,
}

pub struct CoherenceChecker<'a> {
    def_map: &'a glyim_def_map::CrateDefMap,
    registered: HashMap<TraitDefId, Vec<RegisteredImpl>>,
    negative_impls: Vec<(TraitDefId, Ty)>,
}

impl<'a> CoherenceChecker<'a> {
    pub fn new(def_map: &'a glyim_def_map::CrateDefMap) -> Self {
        Self {
            def_map,
            registered: HashMap::new(),
            negative_impls: Vec::new(),
        }
    }

    pub fn check_and_register(
        &mut self,
        header: ResolvedImplHeader,
        ctx: &TyCtxMut,
    ) -> Result<(), Vec<GlyimDiagnostic>> {
        if header.trait_def_id.is_some() || header.trait_name.is_some() {
            self.check_orphan_rule(&header)?;
        }

        if let Some(trait_def_id) = header.trait_def_id
            && let Some(errors) = self.check_overlap(trait_def_id, &header, ctx)
        {
            return Err(errors);
        }

        self.register(header);
        Ok(())
    }

    pub fn check_orphan_rule(
        &self,
        header: &ResolvedImplHeader,
    ) -> Result<(), Vec<GlyimDiagnostic>> {
        // Only true inherent impls (no trait_name AND no trait_def_id) are always local.
        // Unresolved traits (trait_name is Some but trait_def_id is None) are NOT local.
        let trait_is_local = header
            .trait_name
            .and_then(|n| self.def_map.modules[self.def_map.root].scope.resolve(n))
            .is_some()
            || (header.trait_name.is_none() && header.trait_def_id.is_none());

        let self_type_is_local = header
            .self_type_name
            .and_then(|n| self.def_map.modules[self.def_map.root].scope.resolve(n))
            .is_some();

        if trait_is_local || self_type_is_local {
            return Ok(());
        }

        let trait_str = header
            .trait_def_id
            .map(|id| format!("trait #{}", id.to_raw()))
            .unwrap_or_else(|| "<unresolved>".to_string());
        let self_str = format!("{:?}", header.self_ty);

        let msg = format!(
            "orphan rule violation: cannot implement foreign {} for foreign type {}",
            trait_str, self_str,
        );
        Err(vec![GlyimDiagnostic::type_error(header.span, msg)])
    }

    fn self_tys_overlap(
        &self,
        old: &RegisteredImpl,
        new: &ResolvedImplHeader,
        ctx: &TyCtxMut,
    ) -> bool {
        // Direct Ty comparison
        if old.self_ty == new.self_ty {
            return true;
        }

        // Name-based comparison
        if let (Some(a), Some(b)) = (old.self_type_name, new.self_type_name) {
            if a == b {
                return true;
            }
        }

        // Kind-based comparison for when types aren't content-interned
        let old_kind = ctx.ty_kind(old.self_ty);
        let new_kind = ctx.ty_kind(new.self_ty);
        match (old_kind, new_kind) {
            (TyKind::Adt(a, _), TyKind::Adt(b, _)) => a == b,
            (TyKind::Int(a), TyKind::Int(b)) => a == b,
            (TyKind::Uint(a), TyKind::Uint(b)) => a == b,
            (TyKind::Float(a), TyKind::Float(b)) => a == b,
            (TyKind::Param(a), TyKind::Param(b)) => a.index == b.index,
            (TyKind::Bool, TyKind::Bool)
            | (TyKind::Char, TyKind::Char)
            | (TyKind::Never, TyKind::Never)
            | (TyKind::String, TyKind::String) => true,
            _ => false,
        }
    }

    fn check_overlap(
        &self,
        trait_def_id: TraitDefId,
        new_header: &ResolvedImplHeader,
        ctx: &TyCtxMut,
    ) -> Option<Vec<GlyimDiagnostic>> {
        let existing = self.registered.get(&trait_def_id)?;

        for old in existing {
            // Opposite polarities still conflict (can't have both impl and !impl)
            if self.self_tys_overlap(old, new_header, ctx) {
                return Some(self.make_overlap_diag(new_header, old));
            }

            let new_is_blanket = matches!(ctx.ty_kind(new_header.self_ty), TyKind::Param(_));
            let old_is_blanket = matches!(ctx.ty_kind(old.self_ty), TyKind::Param(_));

            // Two blanket impls: conservatively allow if different param names
            if new_is_blanket && old_is_blanket {
                continue;
            }

            // One blanket + one concrete → overlap
            if new_is_blanket || old_is_blanket {
                return Some(self.make_overlap_diag(new_header, old));
            }

            // Generic params might make it a blanket impl
            if !new_header.generic_param_names.is_empty() {
                return Some(self.make_overlap_diag(new_header, old));
            }
        }

        None
    }

    fn make_overlap_diag(
        &self,
        new: &ResolvedImplHeader,
        old: &RegisteredImpl,
    ) -> Vec<GlyimDiagnostic> {
        let trait_str = new
            .trait_def_id
            .map(|id| format!("trait #{}", id.to_raw()))
            .unwrap_or_else(|| "<inherent>".to_string());

        let msg = format!("conflicting implementations of {}", trait_str);
        let mut diag = GlyimDiagnostic::type_error(new.span, msg);
        diag = diag.with_sub(SubDiagnostic {
            severity: DiagSeverity::Note,
            message: "previous impl here".to_string(),
            span: Some(old.span.into()),
        });
        vec![diag]
    }

    /// Compatibility helper for tests — actually uses the polarity parameter.
    #[allow(dead_code)]
    pub(crate) fn check_and_register_impl_compat(
        &mut self,
        header: &ResolvedImplHeader,
        polarity: ImplPolarity,
        ctx: &TyCtxMut,
    ) -> Result<(), Vec<GlyimDiagnostic>> {
        let mut header = header.clone();
        header.polarity = polarity;
        self.check_and_register(header, ctx)
    }

    fn register(&mut self, header: ResolvedImplHeader) {
        let trait_def_id = match header.trait_def_id {
            Some(id) => id,
            None => return,
        };

        let is_blanket = !header.generic_param_names.is_empty();

        let polarity = header.polarity;

        if polarity == ImplPolarity::Negative {
            self.negative_impls.push((trait_def_id, header.self_ty));
        }

        self.registered
            .entry(trait_def_id)
            .or_default()
            .push(RegisteredImpl {
                trait_def_id,
                self_ty: header.self_ty,
                self_type_name: header.self_type_name,
                is_blanket,
                polarity,
                span: header.span,
            });
    }
}
