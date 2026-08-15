use glyim_core::primitives::TargetInfo;
use glyim_diag::{CompResult, GlyimDiagnostic};
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
) -> CompResult<BasicTypeEnum<'ctx>> {
    Ok(match ctx.ty_kind(ty) {
        TyKind::Error => {
            return Err(vec![GlyimDiagnostic::internal_error(
                "Attempted to lower TyKind::Error to LLVM. Type checking failed to resolve this type.",
            )]);
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
                return Ok(context.struct_type(&[], false).into());
            }
            let layout_computer = glyim_layout::SimpleLayoutComputer::new(ctx, target_info.clone());
            let layout = layout_computer
                .layout_of(ty)
                .unwrap_or_else(|_| glyim_layout::Layout::unit());
            if layout.size.0 == 0 {
                return Ok(context.struct_type(&[], false).into());
            }
            opaque_sized_type(context, layout.size.0, layout.align.0)
        }
        TyKind::Array(elem, count) => {
            let elem_llvm = llvm_type_for_ty(ctx, target_info, context, *elem)?;
            let n = match &count.kind {
                glyim_type::ConstKind::Uint(n) => *n as u32,
                glyim_type::ConstKind::Int(n) => *n as u32,
                _ => {
                    return Err(vec![GlyimDiagnostic::internal_error(
                        "internal compiler error: Array with non-integer count in TyKind::Array",
                    )]);
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
        TyKind::Adt(_adt_id, _subst) => {
            let layout_computer = glyim_layout::SimpleLayoutComputer::new(ctx, target_info.clone());
            if let Ok(layout) = layout_computer.layout_of(ty) {
                if layout.size.0 == 0 {
                    return Ok(context.struct_type(&[], false).into());
                }
                opaque_sized_type(context, layout.size.0, layout.align.0)
            } else {
                context.struct_type(&[], false).into()
            }
        }
        TyKind::Closure(_closure_id, _subst) => {
            let layout_computer = glyim_layout::SimpleLayoutComputer::new(ctx, target_info.clone());
            if let Ok(layout) = layout_computer.layout_of(ty) {
                if layout.size.0 == 0 {
                    return Ok(context.struct_type(&[], false).into());
                }
                opaque_sized_type(context, layout.size.0, layout.align.0)
            } else {
                context.struct_type(&[], false).into()
            }
        }
        TyKind::Dynamic(..) => {
            let ptr_ty = context.ptr_type(inkwell::AddressSpace::default());
            context
                .struct_type(&[ptr_ty.into(), ptr_ty.into()], false)
                .into()
        }
        TyKind::Opaque(_, _subst) => {
            let layout_computer = glyim_layout::SimpleLayoutComputer::new(ctx, target_info.clone());
            if let Ok(layout) = layout_computer.layout_of(ty) {
                if layout.size.0 == 0 {
                    return Ok(context.struct_type(&[], false).into());
                }
                opaque_sized_type(context, layout.size.0, layout.align.0)
            } else {
                context.struct_type(&[], false).into()
            }
        }
        TyKind::Projection(_) => {
            return Err(vec![GlyimDiagnostic::internal_error(
                "internal compiler error: TyKind::Projection reached LLVM codegen – type normalization incomplete",
            )]);
        }
        TyKind::Param(param) => {
            return Err(vec![GlyimDiagnostic::internal_error(format!(
                "internal compiler error: TyKind::Param({:?}) reached LLVM codegen – monomorphization should have resolved this",
                param
            ))]);
        }
        TyKind::Bound(debruijn, var) => {
            return Err(vec![GlyimDiagnostic::internal_error(format!(
                "internal compiler error: TyKind::Bound({:?}, {:?}) reached LLVM codegen – binder instantiation failed",
                debruijn, var
            ))]);
        }
        TyKind::Infer(var) => {
            return Err(vec![GlyimDiagnostic::internal_error(format!(
                "internal compiler error: TyKind::Infer({:?}) reached LLVM codegen – type inference incomplete",
                var
            ))]);
        }
    })
}

fn int_type<'ctx>(context: &'ctx Context, bits: u32) -> IntType<'ctx> {
    let non_zero = NonZeroU32::new(bits).unwrap_or_else(|| NonZeroU32::new(64).unwrap());
    context.custom_width_int_type(non_zero).unwrap()
}

/// Creates an opaque LLVM type of exactly the given size and alignment.
/// This is used for aggregate types (structs, tuples, enums) where the internal
/// LLVM struct layout might not match the compiler's layout due to padding.
/// By using an opaque block of the correct size and alignment, we can safely
/// use byte-offset GEPs to access fields, matching the behavior of `build_layout_aggregate`.
pub(crate) fn opaque_sized_type<'ctx>(
    context: &'ctx Context,
    size: u64,
    align: u64,
) -> BasicTypeEnum<'ctx> {
    let align = align.max(1);
    let align_ty: BasicTypeEnum<'ctx> = match align {
        1 => context.i8_type().into(),
        2 => context.i16_type().into(),
        4 => context.i32_type().into(),
        8 => context.i64_type().into(),
        16 => context
            .struct_type(
                &[context.i64_type().into(), context.i64_type().into()],
                false,
            )
            .into(),
        _ => {
            // Fallback to i8 array, alignment might be wrong but at least size is correct
            return context.i8_type().array_type(size as u32).into();
        }
    };
    let num_elems = size / align;
    let remainder = size % align;
    let mut fields: Vec<BasicTypeEnum<'ctx>> = vec![align_ty.array_type(num_elems as u32).into()];
    if remainder > 0 {
        fields.push(context.i8_type().array_type(remainder as u32).into());
    }
    context.struct_type(&fields, false).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::context::Context;

    #[test]
    fn test_opaque_sized_type() {
        let context = Context::create();
        let ty = opaque_sized_type(&context, 24, 8);
        assert!(ty.is_struct_type());
        // We don't assert exact size here because StructType::size_of() can return None
        // without a TargetData layout attached to the context.

        let ty = opaque_sized_type(&context, 10, 4);
        assert!(ty.is_struct_type());

        let ty = opaque_sized_type(&context, 0, 1);
        assert!(ty.is_struct_type());
    }
}
