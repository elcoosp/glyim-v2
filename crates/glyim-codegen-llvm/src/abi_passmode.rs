use crate::types::llvm_type_for_ty;
use glyim_core::primitives::TargetInfo;
use glyim_layout::PassMode;
use glyim_type::{Ty, TyCtx};
use inkwell::builder::Builder;
use inkwell::context::Context;

#[allow(dead_code)]
pub(crate) fn emit_arg_for_pass_mode<'ctx>(
    mode: &PassMode,
    val: inkwell::values::BasicValueEnum<'ctx>,
    ty: Ty,
    ctx: &TyCtx,
    target_info: &TargetInfo,
    context: &'ctx Context,
    builder: &Builder<'ctx>,
) -> Option<inkwell::values::BasicValueEnum<'ctx>> {
    match mode {
        PassMode::Direct => Some(val),
        PassMode::Ignore => None,
        PassMode::Indirect { .. } => {
            let llvm_ty = llvm_type_for_ty(ctx, target_info, context, ty);
            let alloca = builder.build_alloca(llvm_ty, "indirect_arg").ok()?;
            builder.build_store(alloca, val).ok()?;
            Some(alloca.into())
        }
        _ => Some(val),
    }
}
