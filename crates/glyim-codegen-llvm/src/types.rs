use glyim_core::primitives::TargetInfo;
use glyim_layout::LayoutComputer;
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
            panic!("TyKind::Error reached codegen: type checking failed");
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
            let layout_computer = glyim_layout::SimpleLayoutComputer::new(ctx, target_info.clone());
            if let Ok(layout) = layout_computer.layout_of(ty) {
                match &layout.variants {
                    glyim_layout::VariantsShape::Single { .. } => {
                        if let Some(adt_def) = ctx.adt_def(*adt_id) {
                            if let Some(variant) = adt_def.variants.first() {
                                if variant.fields.is_empty() {
                                    return context.struct_type(&[], false).into();
                                }
                                let mut field_types = Vec::with_capacity(variant.fields.len());
                                for field_def in variant.fields.iter() {
                                    field_types.push(llvm_type_for_ty(
                                        ctx,
                                        target_info,
                                        context,
                                        field_def.ty,
                                    ));
                                }
                                return context.struct_type(&field_types, false).into();
                            }
                        }
                        context.struct_type(&[], false).into()
                    }
                    glyim_layout::VariantsShape::Multiple { tag_size, .. } => {
                        let tag_bits = (tag_size.0 * 8) as u32;
                        let tag_ty = int_type(context, tag_bits.max(8));
                        let payload_size = layout.size.0 - tag_size.0;
                        let payload_ty = context.i8_type().array_type(payload_size as u32);
                        context
                            .struct_type(&[tag_ty.into(), payload_ty.into()], false)
                            .into()
                    }
                }
            } else {
                context.struct_type(&[], false).into()
            }
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
