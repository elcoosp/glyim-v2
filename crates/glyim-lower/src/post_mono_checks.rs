use glyim_diag::GlyimDiagnostic;
use glyim_type::TyCtx;

use crate::mono::MonoItemData;

pub(crate) fn check_unsized_locals(
    _items: &[MonoItemData],
    _ctx: &TyCtx,
) -> Vec<GlyimDiagnostic> {
    Vec::new()
}

pub(crate) fn check_large_mono_set(
    _items: &[MonoItemData],
    _threshold: usize,
) -> Vec<GlyimDiagnostic> {
    Vec::new()
}

pub(crate) fn check_unused_generic_params(
    _items: &[MonoItemData],
) -> Vec<GlyimDiagnostic> {
    Vec::new()
}
