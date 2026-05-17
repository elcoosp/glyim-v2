use glyim_core::TargetInfo;
use glyim_type::{Ty, TyCtx, TyKind};
use inkwell::context::Context;
use inkwell::types::{BasicTypeEnum, IntType};
use std::num::NonZeroU32;

pub(crate) fn llvm_type_for_ty<'ctx>(
    ctx: &TyCtx,
    target_info: &TargetInfo,
    context: &'ctx Context,
    ty: Ty,
) -> BasicTypeEnum<'ctx> {
    match ctx.ty_kind(ty) {
        TyKind::Error => {
            tracing::warn!("STUB: error Ty maps to i64");
            int_type(context, 64).into()
        }
        TyKind::Never | TyKind::Unit => context.struct_type(&[], false).into(),
        TyKind::Bool => int_type(context, 1).into(),
        TyKind::Int(it) => {
            let bw = it.bit_width(target_info);
            int_type(context, bw).into()
        }
        TyKind::Uint(ut) => {
            let bw = ut.bit_width(target_info);
            int_type(context, bw).into()
        }
        TyKind::Float(ft) => {
            let bw = ft.bit_width();
            match bw {
                32 => context.f32_type().into(),
                64 => context.f64_type().into(),
                _ => {
                    tracing::warn!("STUB: unknown float width {}", bw);
                    int_type(context, bw).into()
                }
            }
        }
        TyKind::Char => int_type(context, 32).into(),
        TyKind::Ref(..) | TyKind::RawPtr(..) => {
            context.ptr_type(inkwell::AddressSpace::default()).into()
        }
        TyKind::FnPtr(_) | TyKind::FnDef(..) => {
            context.ptr_type(inkwell::AddressSpace::default()).into()
        }
        TyKind::Tuple(_) | TyKind::Array(..) | TyKind::Slice(_) => {
            tracing::warn!("STUB: aggregate type lowered as opaque pointer");
            context.ptr_type(inkwell::AddressSpace::default()).into()
        }
        _ => {
            tracing::warn!("STUB: unknown TyKind {:?} maps to i64", ctx.ty_kind(ty));
            int_type(context, 64).into()
        }
    }
}

fn int_type<'ctx>(context: &'ctx Context, bits: u32) -> IntType<'ctx> {
    context
        .custom_width_int_type(NonZeroU32::new(bits).unwrap_or(NonZeroU32::new(64).unwrap()))
        .unwrap()
}
