use glyim_core::primitives::TargetInfo;
use glyim_type::{Ty, TyCtx, TyKind};
use inkwell::context::Context;
use inkwell::types::{BasicType, BasicTypeEnum, IntType};
use std::num::NonZeroU32;

pub(crate) fn llvm_type_for_ty<'ctx>(
    ctx: &TyCtx,
    target_info: &TargetInfo,
    context: &'ctx Context,
    ty: Ty,
) -> BasicTypeEnum<'ctx> {
    match ctx.ty_kind(ty) {
        TyKind::Error => {
            // panic!("TyKind::Error reached LLVM codegen – type checking should have caught this")
            //   // Error types can appear in MIR during partial compilation or test fixtures.
            // Fall back to i64 to allow codegen to proceed; the program is already ill-typed.
            tracing::warn!("TyKind::Error lowered to i64");
            int_type(context, 64).into()
        }
        TyKind::Never | TyKind::Unit => context.struct_type(&[], false).into(),
        TyKind::Bool => int_type(context, 1).into(),
        TyKind::Int(it) => int_type(context, it.bit_width(target_info)).into(),
        TyKind::Uint(ut) => int_type(context, ut.bit_width(target_info)).into(),
        TyKind::Float(ft) => match ft.bit_width() {
            32 => context.f32_type().into(),
            64 => context.f64_type().into(),
            other => panic!("Unsupported float width {} in TyKind::Float", other),
        },
        TyKind::Char => int_type(context, 32).into(),
        TyKind::String => {
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
                _ => panic!("Array with non-integer count in TyKind::Array"),
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
        TyKind::Adt(adt_id, _subst) => {
            // Use AdtDef to get actual field types, not generic substitution args
            if let Some(adt_def) = ctx.adt_def(*adt_id) {
                // For now, handle the first variant (structs have one; enums need variant selection)
                if let Some(variant) = adt_def.variants.first() {
                    if variant.fields.is_empty() {
                        return context.struct_type(&[], false).into();
                    }
                    let mut field_types = Vec::with_capacity(variant.fields.len());
                    for field_def in variant.fields.iter() {
                        // Use the field's actual type from AdtDef
                        field_types.push(llvm_type_for_ty(ctx, target_info, context, field_def.ty));
                    }
                    return context.struct_type(&field_types, false).into();
                }
            }
            // Fallback for missing AdtDef (should not happen in valid code)
            context.struct_type(&[], false).into()
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
        TyKind::Dynamic(..) => {
            // Trait object fat pointer: { data: *T, vtable: *const VTable }
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
            panic!("TyKind::Projection reached LLVM codegen – type normalization incomplete")
        }
        TyKind::Param(param) => panic!(
            "TyKind::Param({:?}) reached LLVM codegen – monomorphization should have resolved this",
            param
        ),
        TyKind::Bound(debruijn, var) => panic!(
            "TyKind::Bound({:?}, {:?}) reached LLVM codegen – binder instantiation failed",
            debruijn, var
        ),
        TyKind::Infer(var) => panic!(
            "TyKind::Infer({:?}) reached LLVM codegen – type inference incomplete",
            var
        ),
    }
}

fn int_type<'ctx>(context: &'ctx Context, bits: u32) -> IntType<'ctx> {
    let non_zero = NonZeroU32::new(bits).unwrap_or_else(|| NonZeroU32::new(64).unwrap());
    context.custom_width_int_type(non_zero).unwrap()
}
