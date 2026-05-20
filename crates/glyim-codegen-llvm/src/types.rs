use glyim_core::primitives::TargetInfo;
use glyim_type::{Ty, TyCtx, TyKind};
use inkwell::context::Context;
use inkwell::types::{BasicType, BasicTypeEnum, IntType};
use std::num::NonZeroU32;

/// Map a Glyim `Ty` to the corresponding LLVM `BasicTypeEnum`.
///
/// # Preconditions
/// - `ctx` must contain the interned type for `ty`.
/// - `target_info` must reflect the compilation target.
///
/// # Postconditions
/// - Returns a valid LLVM type representing the Glyim type.
/// - `TyKind::Error` maps to `i64` (a visible but safe fallback).
/// - `TyKind::Float(F32)` maps to `f32`, `Float(F64)` maps to `f64`.
/// - Aggregates (tuples, arrays, ADTs) map to LLVM struct/array types.
/// - Unknown `TyKind` variants map to `i64` with a tracing warning.
pub(crate) fn llvm_type_for_ty<'ctx>(
    ctx: &TyCtx,
    target_info: &TargetInfo,
    context: &'ctx Context,
    ty: Ty,
) -> BasicTypeEnum<'ctx> {
    match ctx.ty_kind(ty) {
        TyKind::Error => {
            tracing::debug!("error Ty maps to i64");
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
        TyKind::Float(ft) => match ft.bit_width() {
            32 => context.f32_type().into(),
            64 => context.f64_type().into(),
            other => {
                tracing::warn!(
                    "unsupported float width {} in TyKind::Float, falling back to f64",
                    other
                );
                context.f64_type().into()
            }
        },
        TyKind::Char => int_type(context, 32).into(),
        TyKind::String => {
            // String is a fat pointer: { *u8 data, usize len }
            let ptr_ty = context.ptr_type(inkwell::AddressSpace::default());
            let len_ty = int_type(context, target_info.pointer_width());
            context
                .struct_type(&[ptr_ty.into(), len_ty.into()], false)
                .into()
        }
        TyKind::Ref(..) | TyKind::RawPtr(..) => {
            context.ptr_type(inkwell::AddressSpace::default()).into()
        }
        TyKind::FnPtr(_) | TyKind::FnDef(..) => {
            context.ptr_type(inkwell::AddressSpace::default()).into()
        }
        TyKind::Tuple(subst) => {
            let args = ctx.substitution_args(*subst);
            if args.is_empty() {
                return context.struct_type(&[], false).into();
            }
            let mut field_types = Vec::with_capacity(args.len());
            for arg in args {
                if let glyim_type::GenericArg::Ty(t) = arg {
                    field_types.push(llvm_type_for_ty(ctx, target_info, context, *t));
                }
            }
            if field_types.is_empty() {
                return context.struct_type(&[], false).into();
            }
            context.struct_type(&field_types, false).into()
        }
        TyKind::Array(elem, count) => {
            let elem_llvm = llvm_type_for_ty(ctx, target_info, context, *elem);
            let n = match &count.kind {
                glyim_type::ConstKind::Uint(n) => *n as u32,
                glyim_type::ConstKind::Int(n) => *n as u32,
                _ => {
                    tracing::warn!(
                        "array with non-integer count in TyKind::Array — defaulting to 0"
                    );
                    0
                }
            };
            elem_llvm.array_type(n).into()
        }
        TyKind::Slice(_) => {
            let ptr_ty = context.ptr_type(inkwell::AddressSpace::default());
            let len_ty = int_type(context, target_info.pointer_width());
            context
                .struct_type(&[ptr_ty.into(), len_ty.into()], false)
                .into()
        }
        TyKind::Adt(_adt_id, subst) => {
            // Without AdtDef, treat ADT fields from substitution args
            let args = ctx.substitution_args(*subst);
            if args.is_empty() {
                return context.struct_type(&[], false).into();
            }
            let mut field_types = Vec::with_capacity(args.len());
            for arg in args {
                if let glyim_type::GenericArg::Ty(t) = arg {
                    field_types.push(llvm_type_for_ty(ctx, target_info, context, *t));
                }
            }
            if field_types.is_empty() {
                return context.struct_type(&[], false).into();
            }
            context.struct_type(&field_types, false).into()
        }
        TyKind::Closure(_closure_id, subst) => {
            let args = ctx.substitution_args(*subst);
            if args.is_empty() {
                return context.struct_type(&[], false).into();
            }
            let mut field_types = Vec::with_capacity(args.len());
            for arg in args {
                if let glyim_type::GenericArg::Ty(t) = arg {
                    field_types.push(llvm_type_for_ty(ctx, target_info, context, *t));
                }
            }
            if field_types.is_empty() {
                return context.struct_type(&[], false).into();
            }
            context.struct_type(&field_types, false).into()
        }
        TyKind::Dynamic(_, _) => {
            let ptr_ty = context.ptr_type(inkwell::AddressSpace::default());
            context
                .struct_type(&[ptr_ty.into(), ptr_ty.into()], false)
                .into()
        }
        TyKind::Opaque(_, subst) => {
            let args = ctx.substitution_args(*subst);
            if args.is_empty() {
                return context.struct_type(&[], false).into();
            }
            let mut field_types = Vec::with_capacity(args.len());
            for arg in args {
                if let glyim_type::GenericArg::Ty(t) = arg {
                    field_types.push(llvm_type_for_ty(ctx, target_info, context, *t));
                }
            }
            if field_types.is_empty() {
                return context.struct_type(&[], false).into();
            }
            context.struct_type(&field_types, false).into()
        }
        TyKind::Projection(_) => {
            tracing::debug!("projection Ty maps to i64");
            int_type(context, 64).into()
        }
        TyKind::Param(_) => {
            tracing::debug!("param Ty maps to i64");
            int_type(context, 64).into()
        }
        TyKind::Bound(_, _) => {
            tracing::debug!("bound Ty maps to i64");
            int_type(context, 64).into()
        }
        TyKind::Infer(_) => {
            tracing::debug!("infer Ty maps to i64");
            int_type(context, 64).into()
        }
    }
}

fn int_type<'ctx>(context: &'ctx Context, bits: u32) -> IntType<'ctx> {
    let non_zero = NonZeroU32::new(bits).unwrap_or_else(|| NonZeroU32::new(64).unwrap());
    context.custom_width_int_type(non_zero).unwrap()
}
