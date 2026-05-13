//! Type layout computation
use glyim_core::primitives::TargetInfo;
use glyim_type::TyCtx;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Align(pub u64);

pub struct SimpleLayoutComputer<'a> {
    _ctx: &'a TyCtx,
    _target: TargetInfo,
}
impl<'a> SimpleLayoutComputer<'a> {
    pub fn new(ctx: &'a TyCtx, target: TargetInfo) -> Self {
        Self { _ctx: ctx, _target: target }
    }
}
