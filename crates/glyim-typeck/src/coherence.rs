//! Coherence checker: orphan rules, overlap detection, and negative impls.
//!
//! Enforces that for any `impl Trait for Type`:
//! - At least one of `Trait` or `Type` is defined in the current crate (orphan rule).
//! - The impl does not overlap with any previously registered impl (coherence).
//! - Negative impls (`impl !Trait for Type`) are recorded for auto-trait override.

use std::collections::HashMap;

use glyim_core::def_id::CrateId;
use glyim_core::interner::Name;
use glyim_diag::GlyimDiagnostic;
use glyim_hir::ImplItem;
use glyim_span::Span;
use glyim_type::ImplPolarity;

use glyim_def_map::CrateDefMap;

/// An entry in the registered impls set.
struct RegisteredEntry {
    self_type_name: Name,
    polarity: ImplPolarity,
    /// Names of the impl's generic type parameters (empty for concrete impls).
    generic_param_names: Vec<Name>,
    span: Span,
}

/// Checker for coherence and orphan rules within a single crate.
pub struct CoherenceChecker<'a> {
    def_map: &'a CrateDefMap,
    local_krate: CrateId,
    /// Registered impls: trait name -> list of entries.
    registered: HashMap<Name, Vec<RegisteredEntry>>,
    /// Negative impls: (trait_name, self_type_name)
    negative_impls: Vec<(Name, Name)>,
}

impl<'a> CoherenceChecker<'a> {
    pub fn new(def_map: &'a CrateDefMap, local_krate: CrateId) -> Self {
        Self {
            def_map,
            local_krate,
            registered: HashMap::new(),
            negative_impls: Vec::new(),
        }
    }

    /// Check the orphan rule for the given impl, returning diagnostics on failure.
    pub fn check_orphan_rule(
        &self,
        impl_item: &ImplItem,
        _polarity: ImplPolarity,
    ) -> Result<(), Vec<GlyimDiagnostic>> {
        let trait_is_local = self.trait_is_local(impl_item);
        let self_type_is_local = self.self_type_is_local(impl_item);

        if !trait_is_local && !self_type_is_local {
            let span = Span::DUMMY;
            let msg = format!(
                "orphan rule violation: impl of foreign trait `{}` for foreign type `{}` is not allowed",
                Self::trait_name(impl_item).map(|n| self.resolve_name_str(n)).unwrap_or_default(),
                Self::self_type_name(impl_item).map(|n| self.resolve_name_str(n)).unwrap_or_default()
            );
            return Err(vec![GlyimDiagnostic::type_error(span, msg)]);
        }

        Ok(())
    }

    /// Check orphan rule, check for overlaps, and if all good, register the impl.
    pub fn check_and_register_impl(
        &mut self,
        impl_item: &ImplItem,
        polarity: ImplPolarity,
    ) -> Result<(), Vec<GlyimDiagnostic>> {
        // 1. Orphan check
        self.check_orphan_rule(impl_item, polarity)?;

        // 2. Overlap check
        if let Some(errors) = self.check_overlap(impl_item) {
            return Err(errors);
        }

        // 3. Register
        self.register(impl_item, polarity);

        Ok(())
    }

    /// Check whether we have registered a negative impl for the given trait + type.
    pub fn has_negative_impl(&self, trait_name: &str, type_name: &str) -> bool {
        self.negative_impls
            .iter()
            .any(|(t, s)| {
                let t_str = self.resolve_name_str(*t);
                let s_str = self.resolve_name_str(*s);
                t_str == trait_name && s_str == type_name
            })
    }

    // ---------- private helpers ----------

    fn trait_name(impl_item: &ImplItem) -> Option<Name> {
        impl_item
            .trait_ref
            .as_ref()
            .and_then(|p| p.as_name())
    }

    fn self_type_name(impl_item: &ImplItem) -> Option<Name> {
        match &impl_item.self_ty {
            glyim_hir::TypeRef::Path(p) => p.as_name(),
            _ => None,
        }
    }

    fn resolve_name_str(&self, name: Name) -> String {
        self.def_map.interner.resolve(name).to_string()
    }

    /// Extract the names of generic type parameters from an ImplItem.
    fn generic_param_names(impl_item: &ImplItem) -> Vec<Name> {
        impl_item
            .generic_params
            .iter()
            .map(|p| p.name)
            .collect()
    }

    /// Determine if the trait referenced by the impl is local.
    fn trait_is_local(&self, impl_item: &ImplItem) -> bool {
        if let Some(name) = Self::trait_name(impl_item) {
            let resolved = self.def_map.modules[self.def_map.root]
                .scope
                .resolve(name);
            if resolved.is_some() {
                return true;
            }
        }
        // Inherent impl → treat as local
        impl_item.trait_ref.is_none()
    }

    /// Determine if the self type is local.
    fn self_type_is_local(&self, impl_item: &ImplItem) -> bool {
        if let Some(name) = Self::self_type_name(impl_item) {
            let resolved = self.def_map.modules[self.def_map.root]
                .scope
                .resolve(name);
            if resolved.is_some() {
                return true;
            }
        }
        false
    }

    /// Check for overlapping impls and return diagnostics if conflict found.
    fn check_overlap(&self, new_impl: &ImplItem) -> Option<Vec<GlyimDiagnostic>> {
        let trait_name = Self::trait_name(new_impl)?;
        let self_name = Self::self_type_name(new_impl)?;

        // Gather generic param names of the new impl
        let new_param_names = Self::generic_param_names(new_impl);
        let new_is_blanket = new_param_names.contains(&self_name);

        if let Some(existing) = self.registered.get(&trait_name) {
            for entry in existing {
                // Simple duplicate: same self type name
                if entry.self_type_name == self_name {
                    let msg = format!(
                        "conflicting implementation of trait `{}` for type `{}`",
                        self.resolve_name_str(trait_name),
                        self.resolve_name_str(self_name)
                    );
                    let mut diag = GlyimDiagnostic::type_error(Span::DUMMY, msg);
                    diag = diag.with_sub(
                        glyim_diag::SubDiagnostic {
                            severity: glyim_diag::DiagSeverity::Note,
                            message: "previous impl here".to_string(),
                            span: Some(entry.span.into()),
                        },
                    );
                    return Some(vec![diag]);
                }

                // Blanket overlap: if one is blanket and the other concrete
                let existing_is_blanket = entry.generic_param_names.contains(&entry.self_type_name);
                if new_is_blanket != existing_is_blanket {
                    // Conflict
                    let msg = format!(
                        "conflicting implementations of trait `{}`",
                        self.resolve_name_str(trait_name)
                    );
                    let mut diag = GlyimDiagnostic::type_error(Span::DUMMY, msg);
                    diag = diag.with_sub(
                        glyim_diag::SubDiagnostic {
                            severity: glyim_diag::DiagSeverity::Note,
                            message: "blanket impl conflicts with concrete impl".to_string(),
                            span: None,
                        },
                    );
                    return Some(vec![diag]);
                }
            }
        }

        None
    }

    fn register(&mut self, impl_item: &ImplItem, polarity: ImplPolarity) {
        let trait_name = match Self::trait_name(impl_item) {
            Some(n) => n,
            None => return,
        };
        let self_name = match Self::self_type_name(impl_item) {
            Some(n) => n,
            None => return,
        };

        let entry = RegisteredEntry {
            self_type_name: self_name,
            polarity,
            generic_param_names: Self::generic_param_names(impl_item),
            span: Span::DUMMY,
        };

        self.registered
            .entry(trait_name)
            .or_default()
            .push(entry);

        if polarity == ImplPolarity::Negative {
            self.negative_impls.push((trait_name, self_name));
        }
    }
}
