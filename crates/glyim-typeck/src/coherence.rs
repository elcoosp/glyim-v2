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
        if header.trait_def_id.is_some() {
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

    fn check_orphan_rule(&self, header: &ResolvedImplHeader) -> Result<(), Vec<GlyimDiagnostic>> {
        let trait_is_local = header
            .trait_name
            .and_then(|n| self.def_map.modules[self.def_map.root].scope.resolve(n))
            .is_some()
            || header.trait_def_id.is_none(); // Inherent impls are always local

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
            .unwrap_or_else(|| "<inherent>".to_string());
        let self_str = format!("{:?}", header.self_ty);

        let msg = format!(
            "orphan rule violation: cannot implement foreign {} for foreign type {}",
            trait_str, self_str,
        );
        Err(vec![GlyimDiagnostic::type_error(header.span, msg)])
    }

    fn check_overlap(
        &self,
        trait_def_id: TraitDefId,
        new_header: &ResolvedImplHeader,
        ctx: &TyCtxMut,
    ) -> Option<Vec<GlyimDiagnostic>> {
        let existing = self.registered.get(&trait_def_id)?;

        for old in existing {
            if old.polarity != new_header.polarity {
                continue;
            }

            // If self types are exactly the same, they overlap.
            if old.self_ty == new_header.self_ty {
                return Some(self.make_overlap_diag(new_header, old));
            }

            // If either is explicitly a TyKind::Param, it's a blanket impl.
            let new_is_blanket = matches!(ctx.ty_kind(new_header.self_ty), TyKind::Param(_));
            let old_is_blanket = matches!(ctx.ty_kind(old.self_ty), TyKind::Param(_));

            if new_is_blanket || old_is_blanket {
                return Some(self.make_overlap_diag(new_header, old));
            }

            // Conservative: if it has generic params, it might be a blanket impl
            // e.g., impl<T> Foo for Vec<T>.
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

    fn register(&mut self, header: ResolvedImplHeader) {
        let trait_def_id = match header.trait_def_id {
            Some(id) => id,
            None => return,
        };

        // If we can't deeply check Substitution, assume generic_param_names implies blanket
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
                is_blanket,
                polarity,
                span: header.span,
            });
    }
}
