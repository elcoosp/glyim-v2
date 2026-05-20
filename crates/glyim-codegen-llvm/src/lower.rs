use crate::abi::FullLayoutComputer;
use crate::debug::DebugInfoCtx;
use crate::types::llvm_type_for_ty;
use glyim_core::TargetInfo;
use glyim_core::arena::IndexVec;
use glyim_core::primitives::*;
use glyim_diag::{CompResult, GlyimDiagnostic};
use glyim_layout::{LayoutComputer, PassMode};
use glyim_mir::{
    AggregateKind, BasicBlockIdx, Body, CastKind, LocalIdx, MirConst, MirConstKind, Operand, Place,
    ProjectionElem, Rvalue, Statement, StatementKind, Terminator, TerminatorKind,
};
use glyim_span::{FileId, Span};
use glyim_type::{ConstKind, Ty, TyCtx, TyKind};
use inkwell::AddressSpace;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::BasicType;
use inkwell::values::{AnyValue, AnyValueEnum, BasicValue, BasicValueEnum, IntValue, PointerValue};
use std::collections::HashMap;
use std::num::NonZeroU32;

fn local_ty(body: &Body, local: LocalIdx) -> Ty {
    body.locals[local].ty
}

struct LoweringCtx<'ctx, 'a> {
    context: &'ctx Context,
    builder: Builder<'ctx>,
    function: inkwell::values::FunctionValue<'ctx>,
    module: &'a Module<'ctx>,
    drop_fn: inkwell::values::FunctionValue<'ctx>,
    dealloc_fn: inkwell::values::FunctionValue<'ctx>,
    body: &'a Body,
    target_info: TargetInfo,
    ty_ctx: &'a TyCtx,
    locals: IndexVec<LocalIdx, Option<PointerValue<'ctx>>>,
    bb_map: HashMap<BasicBlockIdx, inkwell::basic_block::BasicBlock<'ctx>>,
    personality_fn: Option<inkwell::values::FunctionValue<'ctx>>,
    debug_ctx: Option<DebugInfoCtx<'ctx>>,
}

impl<'ctx, 'a> LoweringCtx<'ctx, 'a> {
    fn llvm_int_type(&self, bits: u32) -> inkwell::types::IntType<'ctx> {
        let non_zero = NonZeroU32::new(bits).unwrap_or_else(|| NonZeroU32::new(64).unwrap());
        self.context.custom_width_int_type(non_zero).unwrap()
    }

    fn llvm_type_for_ty(&self, ty: Ty) -> inkwell::types::BasicTypeEnum<'ctx> {
        llvm_type_for_ty(self.ty_ctx, &self.target_info, self.context, ty)
    }

    fn set_debug_location(&self, span: Span) {
        if span.is_dummy() {
            return;
        }
        if let Some(ref debug_ctx) = self.debug_ctx
            && let Some(loc) = debug_ctx.location_for_span(self.context, &span)
        {
            self.builder.set_current_debug_location(loc);
        }
    }

    fn clear_debug_location(&self) {
        if self.debug_ctx.is_some() {
            // No direct clear, but setting to a dummy location is not needed.
            // Leave as is; the next set_debug_location will overwrite.
        }
    }

    fn alloc_local(&mut self, local: LocalIdx) {
        let ty = local_ty(self.body, local);
        let llvm_ty = self.llvm_type_for_ty(ty);
        let name = format!("local_{}", local.index());

        // Zero-sized types (unit, never, empty struct) get a null pointer
        let is_zero_sized = ty == Ty::UNIT
            || ty == Ty::NEVER
            || (if let inkwell::types::BasicTypeEnum::StructType(st) = llvm_ty {
                st.get_field_types().is_empty()
            } else {
                false
            });

        if is_zero_sized {
            let ptr = self
                .context
                .ptr_type(inkwell::AddressSpace::default())
                .const_null();
            self.locals[local] = Some(ptr);
        } else {
            let alloca = self
                .builder
                .build_alloca(llvm_ty, &name)
                .expect("alloca failed");
            self.locals[local] = Some(alloca);
        }
    }

    fn get_local_ptr(&self, local: LocalIdx) -> PointerValue<'ctx> {
        self.locals[local].unwrap_or_else(|| panic!("local {} not allocated", local.index()))
    }

    fn lower_operand(&self, operand: &Operand) -> BasicValueEnum<'ctx> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => {
                let ptr = self.place_ptr(place);
                let ty = self.place_ty(place);
                let llvm_ty = self.llvm_type_for_ty(ty);
                self.builder
                    .build_load(llvm_ty, ptr, "load")
                    .expect("load failed")
            }
            Operand::Constant(c) => self.lower_const(c),
        }
    }

    fn lower_const(&self, c: &MirConst) -> BasicValueEnum<'ctx> {
        match &c.kind {
            MirConstKind::Int(v) => {
                let ty = self.llvm_type_for_ty(c.ty);
                let int_ty = ty.into_int_type();
                int_ty.const_int(*v as u64, true).into()
            }
            MirConstKind::Uint(v) => {
                let ty = self.llvm_type_for_ty(c.ty);
                let int_ty = ty.into_int_type();
                int_ty.const_int(*v as u64, false).into()
            }
            MirConstKind::Bool(b) => self
                .llvm_int_type(1)
                .const_int(if *b { 1 } else { 0 }, false)
                .into(),
            MirConstKind::FloatBits(bits) => {
                let ty = self.llvm_type_for_ty(c.ty);
                let float_ty = ty.into_float_type();
                float_ty.const_float(f64::from_bits(*bits)).into()
            }
            MirConstKind::Char(ch) => self.llvm_int_type(32).const_int(*ch as u64, false).into(),
            MirConstKind::Unit => {
                let unit_ty = self.context.struct_type(&[], false);
                unit_ty.const_zero().as_basic_value_enum()
            }
            MirConstKind::String(name) => {
                let str_content = self.ty_ctx.name_str(*name);
                let module = self.module;
                let safe_name: String = str_content
                    .chars()
                    .take(32)
                    .map(|c| if c.is_alphanumeric() { c } else { '_' })
                    .collect();
                let global_name = format!("__glyim_str_{}", safe_name);
                let global = if let Some(existing) = module.get_global(&global_name) {
                    existing
                } else {
                    let const_str = self.context.const_string(str_content.as_bytes(), true);
                    let i8_type = self.context.i8_type();
                    let str_type = i8_type.array_type(str_content.len() as u32 + 1);
                    let global = module.add_global(
                        str_type,
                        Some(inkwell::AddressSpace::default()),
                        &global_name,
                    );
                    global.set_initializer(&const_str);
                    global.set_constant(true);
                    global.set_linkage(inkwell::module::Linkage::Private);
                    global
                };
                let ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
                self.builder
                    .build_bit_cast(global.as_pointer_value(), ptr_type, "str_ptr")
                    .expect("bitcast for string constant failed")
            }
            MirConstKind::Fn(fn_def_id, _substs) => {
                let fn_name = format!("__glyim_fn_{}", fn_def_id.to_raw());
                let module = self.module;
                let callee = module.get_function(&fn_name).unwrap_or_else(|| {
                    let fn_type = self.context.void_type().fn_type(&[], false);
                    module.add_function(&fn_name, fn_type, None)
                });
                callee
                    .as_global_value()
                    .as_pointer_value()
                    .as_basic_value_enum()
            }
            MirConstKind::ConstRef(const_def_id, _substs) => {
                let global_name = format!("__glyim_const_{}", const_def_id.to_raw());
                let module = self.module;
                let global = module.get_global(&global_name).unwrap_or_else(|| {
                    let llvm_ty = self.llvm_type_for_ty(c.ty);
                    module.add_global(
                        llvm_ty,
                        Some(inkwell::AddressSpace::default()),
                        &global_name,
                    )
                });
                let llvm_ty = self.llvm_type_for_ty(c.ty);
                self.builder
                    .build_load(llvm_ty, global.as_pointer_value(), "const_ref_load")
                    .expect("const ref load failed")
            }
            MirConstKind::Error => {
                tracing::debug!("error constant lowered as i64 zero");
                self.llvm_int_type(64).const_zero().into()
            }
        }
    }

    fn place_ptr(&self, place: &Place) -> PointerValue<'ctx> {
        let base = self.get_local_ptr(place.local);
        if place.projection.is_empty() {
            return base;
        }
        let mut ptr = base;
        let mut current_ty = local_ty(self.body, place.local);
        for elem in place.projection.iter() {
            match elem {
                ProjectionElem::Deref => {
                    let llvm_ty = self.llvm_type_for_ty(current_ty);
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, ptr, "deref_load")
                        .expect("deref load failed");
                    ptr = loaded.into_pointer_value();
                    current_ty = match self.ty_ctx.ty_kind(current_ty) {
                        TyKind::Ref(_, inner, _) | TyKind::RawPtr(inner, _) => *inner,
                        other => {
                            tracing::debug!(
                                "Deref on non-pointer type {:?}, treating as pointer",
                                other
                            );
                            let llvm_ty = self.llvm_type_for_ty(current_ty);
                            let loaded = self
                                .builder
                                .build_load(llvm_ty, ptr, "deref_load_nonptr")
                                .expect("deref load failed");
                            ptr = loaded.into_pointer_value();
                            current_ty
                        }
                    };
                }
                ProjectionElem::Field(idx) => {
                    let field_idx = idx.to_raw() as u64;
                    let i32_type = self.llvm_int_type(32);
                    let zero = i32_type.const_zero();
                    let field_index = i32_type.const_int(field_idx, false);

                    let llvm_ty = self.llvm_type_for_ty(current_ty);
                    ptr = unsafe {
                        self.builder
                            .build_in_bounds_gep(llvm_ty, ptr, &[zero, field_index], "field_gep")
                            .expect("field gep failed")
                    };

                    current_ty = match self.ty_ctx.ty_kind(current_ty) {
                        TyKind::Tuple(subst) => {
                            let args = self.ty_ctx.substitution_args(*subst);
                            args.get(idx.to_raw() as usize)
                                .and_then(|arg| {
                                    if let glyim_type::GenericArg::Ty(t) = arg {
                                        Some(*t)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(Ty::ERROR)
                        }
                        TyKind::Adt(_adt_id, subst) => {
                            let args = self.ty_ctx.substitution_args(*subst);
                            args.get(idx.to_raw() as usize)
                                .and_then(|arg| {
                                    if let glyim_type::GenericArg::Ty(t) = arg {
                                        Some(*t)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(Ty::ERROR)
                        }
                        TyKind::Closure(_, subst) => {
                            let args = self.ty_ctx.substitution_args(*subst);
                            args.get(idx.to_raw() as usize)
                                .and_then(|arg| {
                                    if let glyim_type::GenericArg::Ty(t) = arg {
                                        Some(*t)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(Ty::ERROR)
                        }
                        other => {
                            tracing::debug!(
                                "field projection on non-aggregate type {:?}, returning error type",
                                other
                            );
                            Ty::ERROR
                        }
                    };
                }
                ProjectionElem::Index(local_idx) => {
                    let index_ptr = self.get_local_ptr(*local_idx);
                    let i64_ty = self.llvm_int_type(64);
                    let index_val = self
                        .builder
                        .build_load(i64_ty, index_ptr, "index_load")
                        .expect("index load failed")
                        .into_int_value();
                    let i32_ty = self.llvm_int_type(32);
                    let truncated = self
                        .builder
                        .build_int_truncate(index_val, i32_ty, "idx_trunc")
                        .expect("idx trunc failed");

                    let elem_ty = match self.ty_ctx.ty_kind(current_ty) {
                        TyKind::Array(elem, _) => *elem,
                        TyKind::Slice(elem) => *elem,
                        other => {
                            tracing::debug!(
                                "index projection on non-array/slice type {:?}, returning error type",
                                other
                            );
                            Ty::ERROR
                        }
                    };
                    let llvm_ty = self.llvm_type_for_ty(current_ty);
                    ptr = unsafe {
                        self.builder
                            .build_in_bounds_gep(
                                llvm_ty,
                                ptr,
                                &[i32_ty.const_zero(), truncated],
                                "index_gep",
                            )
                            .expect("index gep failed")
                    };
                    current_ty = elem_ty;
                }
                ProjectionElem::Downcast(variant_idx) => {
                    tracing::debug!("Downcast projection to variant {}", variant_idx.to_raw());
                }
            }
        }
        ptr
    }

    fn place_ty(&self, place: &Place) -> Ty {
        local_ty(self.body, place.local)
    }

    fn emit_landingpad(&self) -> CompResult<()> {
        if let Some(personality_fn) = self.personality_fn {
            let ptr_type = self.context.ptr_type(AddressSpace::default());
            let i32_type = self.context.i32_type();
            let result_type = self
                .context
                .struct_type(&[ptr_type.into(), i32_type.into()], false);

            let _pad = self
                .builder
                .build_landing_pad(result_type, personality_fn, &[], true, "pad")
                .map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "landingpad: {:?}",
                        e
                    ))]
                })?;
        }
        Ok(())
    }

    fn lower_rvalue(&mut self, rvalue: &Rvalue) -> BasicValueEnum<'ctx> {
        match rvalue {
            Rvalue::Use(operand) => self.lower_operand(operand),
            Rvalue::Ref(place, _borrow_kind) => {
                let ptr = self.place_ptr(place);
                ptr.as_basic_value_enum()
            }
            Rvalue::BinaryOp(op, operands) => {
                let lhs_val = self.lower_operand(&operands.0);
                let rhs_val = self.lower_operand(&operands.1);
                let operand_ty = self.operand_ty(&operands.0);
                let is_float = matches!(self.ty_ctx.ty_kind(operand_ty), TyKind::Float(_));
                let is_unsigned = matches!(self.ty_ctx.ty_kind(operand_ty), TyKind::Uint(_));
                if is_float {
                    let lhs = lhs_val.into_float_value();
                    let rhs = rhs_val.into_float_value();
                    self.lower_float_binary_op(*op, lhs, rhs)
                } else {
                    let lhs = lhs_val.into_int_value();
                    let rhs = rhs_val.into_int_value();
                    self.lower_binary_op(*op, lhs, rhs, is_unsigned)
                }
            }
            Rvalue::UnaryOp(op, operand) => {
                let val = self.lower_operand(operand);
                let operand_ty = self.operand_ty(operand);
                if matches!(self.ty_ctx.ty_kind(operand_ty), TyKind::Float(_)) {
                    let float_val = val.into_float_value();
                    self.lower_float_unary_op(*op, float_val)
                } else {
                    let int_val = val.into_int_value();
                    self.lower_unary_op(*op, int_val)
                }
            }
            Rvalue::Aggregate(kind, operands) => self.lower_aggregate(kind, operands),
            Rvalue::Discriminant(place) => self.lower_discriminant(place),
            Rvalue::Len(place) => self.lower_len(place),
            Rvalue::Cast(cast_kind, operand, ty) => self.lower_cast(*cast_kind, operand, *ty),
            Rvalue::Repeat(operand, count) => self.lower_repeat(operand, count),
        }
    }

    fn lower_aggregate(
        &mut self,
        kind: &AggregateKind,
        operands: &[Operand],
    ) -> BasicValueEnum<'ctx> {
        match kind {
            AggregateKind::Tuple => {
                let field_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = operands
                    .iter()
                    .map(|op| self.operand_ty(op))
                    .map(|ty| self.llvm_type_for_ty(ty))
                    .collect();
                let struct_ty = self.context.struct_type(&field_types, false);
                let mut result = struct_ty.const_zero().as_basic_value_enum();
                for (i, op) in operands.iter().enumerate() {
                    let val = self.lower_operand(op);
                    let agg = result.into_struct_value();
                    let inserted = self
                        .builder
                        .build_insert_value(agg, val, i as u32, "insert_field")
                        .expect("insert_value failed for tuple field");
                    result = inserted.as_basic_value_enum();
                }
                result
            }
            AggregateKind::Array(elem_ty) => {
                let elem_llvm_ty = self.llvm_type_for_ty(*elem_ty);
                let array_ty = elem_llvm_ty.array_type(operands.len().try_into().unwrap_or(0u32));
                let mut result = array_ty.const_zero().as_basic_value_enum();
                for (i, op) in operands.iter().enumerate() {
                    let val = self.lower_operand(op);
                    let agg = result.into_array_value();
                    let inserted = self
                        .builder
                        .build_insert_value(agg, val, i as u32, "insert_elem")
                        .expect("insert_value failed for array element");
                    result = inserted.as_basic_value_enum();
                }
                result
            }
            AggregateKind::Adt(_adt_id, _variant_idx, _substs) => {
                let field_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = operands
                    .iter()
                    .map(|op| self.operand_ty(op))
                    .map(|ty| self.llvm_type_for_ty(ty))
                    .collect();
                if field_types.is_empty() {
                    let unit_struct = self.context.struct_type(&[], false);
                    return unit_struct.const_zero().as_basic_value_enum();
                }
                let struct_ty = self.context.struct_type(&field_types, false);
                let mut result = struct_ty.const_zero().as_basic_value_enum();
                for (i, op) in operands.iter().enumerate() {
                    let val = self.lower_operand(op);
                    let agg = result.into_struct_value();
                    let inserted = self
                        .builder
                        .build_insert_value(agg, val, i as u32, "insert_adt_field")
                        .expect("insert_value failed for ADT field");
                    result = inserted.as_basic_value_enum();
                }
                result
            }
            AggregateKind::Closure(_closure_id, _substs) => {
                let field_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = operands
                    .iter()
                    .map(|op| self.operand_ty(op))
                    .map(|ty| self.llvm_type_for_ty(ty))
                    .collect();
                if field_types.is_empty() {
                    let unit_struct = self.context.struct_type(&[], false);
                    return unit_struct.const_zero().as_basic_value_enum();
                }
                let struct_ty = self.context.struct_type(&field_types, false);
                let mut result = struct_ty.const_zero().as_basic_value_enum();
                for (i, op) in operands.iter().enumerate() {
                    let val = self.lower_operand(op);
                    let agg = result.into_struct_value();
                    let inserted = self
                        .builder
                        .build_insert_value(agg, val, i as u32, "insert_closure_field")
                        .expect("insert_value failed for Closure field");
                    result = inserted.as_basic_value_enum();
                }
                result
            }
        }
    }

    fn lower_discriminant(&self, place: &Place) -> BasicValueEnum<'ctx> {
        let ptr = self.place_ptr(place);
        let place_ty = self.place_ty(place);

        match self.ty_ctx.ty_kind(place_ty) {
            TyKind::Bool => {
                let val = self
                    .builder
                    .build_load(self.llvm_int_type(1), ptr, "discr_load")
                    .expect("discr load failed");
                self.builder
                    .build_int_z_extend(val.into_int_value(), self.llvm_int_type(32), "discr_ext")
                    .expect("discr ext failed")
                    .into()
            }
            _ => {
                tracing::warn!("discriminant on type without AdtDef — emitting 0 as safe sentinel");
                self.llvm_int_type(32).const_int(0, false).into()
            }
        }
    }

    fn lower_len(&self, place: &Place) -> BasicValueEnum<'ctx> {
        let place_ty = self.place_ty(place);
        match self.ty_ctx.ty_kind(place_ty) {
            TyKind::Array(_elem, count) => {
                let n = match &count.kind {
                    ConstKind::Uint(n) => *n as u64,
                    ConstKind::Int(n) => *n as u64,
                    other => {
                        tracing::warn!(
                            "Len with non-integer count {:?} — emitting 0 as safe sentinel",
                            other
                        );
                        0
                    }
                };
                let len_ty = self.llvm_int_type(64);
                len_ty.const_int(n, false).into()
            }
            TyKind::Slice(_) => {
                let ptr = self.place_ptr(place);
                let i64_ty = self.llvm_int_type(64);
                let i32_ty = self.llvm_int_type(32);
                let slice_ty = self.llvm_type_for_ty(place_ty);
                let len_ptr = unsafe {
                    self.builder
                        .build_in_bounds_gep(
                            slice_ty,
                            ptr,
                            &[i32_ty.const_zero(), i32_ty.const_int(1, false)],
                            "len_gep",
                        )
                        .expect("len gep failed")
                };
                self.builder
                    .build_load(i64_ty, len_ptr, "len_load")
                    .expect("len load failed")
            }
            other => {
                tracing::warn!(
                    "Len on non-array/slice type {:?} — emitting 0 as safe sentinel",
                    other
                );
                self.llvm_int_type(64).const_zero().into()
            }
        }
    }

    fn lower_binary_op(
        &self,
        op: BinOp,
        lhs: IntValue<'ctx>,
        rhs: IntValue<'ctx>,
        is_unsigned: bool,
    ) -> BasicValueEnum<'ctx> {
        match op {
            BinOp::Add => self
                .builder
                .build_int_add(lhs, rhs, "add")
                .expect("add failed")
                .into(),
            BinOp::Sub => self
                .builder
                .build_int_sub(lhs, rhs, "sub")
                .expect("sub failed")
                .into(),
            BinOp::Mul => self
                .builder
                .build_int_mul(lhs, rhs, "mul")
                .expect("mul failed")
                .into(),
            BinOp::Div => {
                if is_unsigned {
                    self.builder
                        .build_int_unsigned_div(lhs, rhs, "udiv")
                        .expect("udiv failed")
                        .into()
                } else {
                    self.builder
                        .build_int_signed_div(lhs, rhs, "sdiv")
                        .expect("sdiv failed")
                        .into()
                }
            }
            BinOp::Rem => {
                if is_unsigned {
                    self.builder
                        .build_int_unsigned_rem(lhs, rhs, "urem")
                        .expect("urem failed")
                        .into()
                } else {
                    self.builder
                        .build_int_signed_rem(lhs, rhs, "srem")
                        .expect("srem failed")
                        .into()
                }
            }
            BinOp::Eq => self
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, lhs, rhs, "eq")
                .expect("eq failed")
                .into(),
            BinOp::Ne => self
                .builder
                .build_int_compare(inkwell::IntPredicate::NE, lhs, rhs, "ne")
                .expect("ne failed")
                .into(),
            BinOp::Lt => {
                let pred = if is_unsigned {
                    inkwell::IntPredicate::ULT
                } else {
                    inkwell::IntPredicate::SLT
                };
                self.builder
                    .build_int_compare(pred, lhs, rhs, "lt")
                    .expect("lt failed")
                    .into()
            }
            BinOp::Gt => {
                let pred = if is_unsigned {
                    inkwell::IntPredicate::UGT
                } else {
                    inkwell::IntPredicate::SGT
                };
                self.builder
                    .build_int_compare(pred, lhs, rhs, "gt")
                    .expect("gt failed")
                    .into()
            }
            BinOp::LtEq => {
                let pred = if is_unsigned {
                    inkwell::IntPredicate::ULE
                } else {
                    inkwell::IntPredicate::SLE
                };
                self.builder
                    .build_int_compare(pred, lhs, rhs, "le")
                    .expect("le failed")
                    .into()
            }
            BinOp::GtEq => {
                let pred = if is_unsigned {
                    inkwell::IntPredicate::UGE
                } else {
                    inkwell::IntPredicate::SGE
                };
                self.builder
                    .build_int_compare(pred, lhs, rhs, "ge")
                    .expect("ge failed")
                    .into()
            }
            BinOp::And => self
                .builder
                .build_and(lhs, rhs, "and")
                .expect("and failed")
                .into(),
            BinOp::Or => self
                .builder
                .build_or(lhs, rhs, "or")
                .expect("or failed")
                .into(),
            BinOp::BitAnd => self
                .builder
                .build_and(lhs, rhs, "bitand")
                .expect("bitand failed")
                .into(),
            BinOp::BitOr => self
                .builder
                .build_or(lhs, rhs, "bitor")
                .expect("bitor failed")
                .into(),
            BinOp::BitXor => self
                .builder
                .build_xor(lhs, rhs, "bitxor")
                .expect("bitxor failed")
                .into(),
            BinOp::Shl => self
                .builder
                .build_left_shift(lhs, rhs, "shl")
                .expect("shl failed")
                .into(),
            BinOp::Shr => self
                .builder
                .build_right_shift(lhs, rhs, !is_unsigned, "shr")
                .expect("shr failed")
                .into(),
        }
    }

    fn lower_float_binary_op(
        &self,
        op: BinOp,
        lhs: inkwell::values::FloatValue<'ctx>,
        rhs: inkwell::values::FloatValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        match op {
            BinOp::Add => self
                .builder
                .build_float_add(lhs, rhs, "fadd")
                .expect("fadd failed")
                .into(),
            BinOp::Sub => self
                .builder
                .build_float_sub(lhs, rhs, "fsub")
                .expect("fsub failed")
                .into(),
            BinOp::Mul => self
                .builder
                .build_float_mul(lhs, rhs, "fmul")
                .expect("fmul failed")
                .into(),
            BinOp::Div => self
                .builder
                .build_float_div(lhs, rhs, "fdiv")
                .expect("fdiv failed")
                .into(),
            BinOp::Rem => self
                .builder
                .build_float_rem(lhs, rhs, "frem")
                .expect("frem failed")
                .into(),
            BinOp::Eq => self
                .builder
                .build_float_compare(inkwell::FloatPredicate::OEQ, lhs, rhs, "feq")
                .expect("feq failed")
                .into(),
            BinOp::Ne => self
                .builder
                .build_float_compare(inkwell::FloatPredicate::ONE, lhs, rhs, "fne")
                .expect("fne failed")
                .into(),
            BinOp::Lt => self
                .builder
                .build_float_compare(inkwell::FloatPredicate::OLT, lhs, rhs, "flt")
                .expect("flt failed")
                .into(),
            BinOp::Gt => self
                .builder
                .build_float_compare(inkwell::FloatPredicate::OGT, lhs, rhs, "fgt")
                .expect("fgt failed")
                .into(),
            BinOp::LtEq => self
                .builder
                .build_float_compare(inkwell::FloatPredicate::OLE, lhs, rhs, "fle")
                .expect("fle failed")
                .into(),
            BinOp::GtEq => self
                .builder
                .build_float_compare(inkwell::FloatPredicate::OGE, lhs, rhs, "fge")
                .expect("fge failed")
                .into(),
            other => {
                tracing::warn!(
                    "unsupported float binary op {:?} — returning lhs as safe sentinel",
                    other
                );
                lhs.into()
            }
        }
    }

    fn lower_float_unary_op(
        &self,
        op: UnOp,
        val: inkwell::values::FloatValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        match op {
            UnOp::Neg => self
                .builder
                .build_float_neg(val, "fneg")
                .expect("fneg failed")
                .into(),
            UnOp::Not => {
                tracing::debug!("logical Not on float is undefined, emitting trap");
                let module = self.module;
                let trap_fn_name = "llvm.trap";
                let trap_fn = module.get_function(trap_fn_name).unwrap_or_else(|| {
                    module.add_function(
                        trap_fn_name,
                        self.context.void_type().fn_type(&[], false),
                        None,
                    )
                });
                self.builder
                    .build_call(trap_fn, &[], "float_not_trap")
                    .expect("trap call failed");
                self.builder
                    .build_unreachable()
                    .expect("unreachable failed");
                val.into()
            }
            UnOp::Deref => {
                tracing::debug!("Deref on float is undefined, emitting trap");
                let module = self.module;
                let trap_fn_name = "llvm.trap";
                let trap_fn = module.get_function(trap_fn_name).unwrap_or_else(|| {
                    module.add_function(
                        trap_fn_name,
                        self.context.void_type().fn_type(&[], false),
                        None,
                    )
                });
                self.builder
                    .build_call(trap_fn, &[], "float_deref_trap")
                    .expect("trap call failed");
                self.builder
                    .build_unreachable()
                    .expect("unreachable failed");
                val.into()
            }
        }
    }

    fn lower_unary_op(&self, op: UnOp, val: IntValue<'ctx>) -> BasicValueEnum<'ctx> {
        match op {
            UnOp::Not => self
                .builder
                .build_not(val, "not")
                .expect("not failed")
                .into(),
            UnOp::Neg => self
                .builder
                .build_int_neg(val, "neg")
                .expect("neg failed")
                .into(),
            UnOp::Deref => {
                // Deref on an integer-typed value: load through the pointer it represents.
                // The integer is treated as a raw pointer address; emit a load of i64.
                tracing::debug!("UnaryOp::Deref on integer value — emitting inttoptr + load");
                let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                let ptr = self
                    .builder
                    .build_int_to_ptr(val, ptr_ty, "deref_inttoptr")
                    .expect("inttoptr failed");
                let i64_ty = self.llvm_int_type(64);
                self.builder
                    .build_load(i64_ty, ptr, "deref_load")
                    .expect("deref load failed")
            }
        }
    }

    fn operand_ty(&self, operand: &Operand) -> Ty {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => self.place_ty(place),
            Operand::Constant(c) => c.ty,
        }
    }

    fn lower_cast(&self, kind: CastKind, operand: &Operand, target_ty: Ty) -> BasicValueEnum<'ctx> {
        let val = self.lower_operand(operand);
        match kind {
            CastKind::IntToInt => {
                let int_val = val.into_int_value();
                let dest_type = self.llvm_type_for_ty(target_ty).into_int_type();
                let dest_bits = dest_type.get_bit_width();
                let src_bits = int_val.get_type().get_bit_width();
                if src_bits < dest_bits {
                    self.builder
                        .build_int_s_extend(int_val, dest_type, "int_to_int_ext")
                        .expect("ext failed")
                        .into()
                } else if src_bits > dest_bits {
                    self.builder
                        .build_int_truncate(int_val, dest_type, "int_to_int_trunc")
                        .expect("trunc failed")
                        .into()
                } else {
                    val
                }
            }
            CastKind::FloatToInt => {
                let float_val = val.into_float_value();
                let dest_type = self.llvm_type_for_ty(target_ty).into_int_type();
                self.builder
                    .build_float_to_signed_int(float_val, dest_type, "float_to_int")
                    .expect("f2i failed")
                    .into()
            }
            CastKind::IntToFloat => {
                let int_val = val.into_int_value();
                let dest_type = self.llvm_type_for_ty(target_ty).into_float_type();
                self.builder
                    .build_signed_int_to_float(int_val, dest_type, "int_to_float")
                    .expect("i2f failed")
                    .into()
            }
            CastKind::PtrToPtr => val,
            CastKind::FnPtrToPtr => val,
        }
    }

    fn lower_repeat(&self, operand: &Operand, count: &MirConst) -> BasicValueEnum<'ctx> {
        let val = self.lower_operand(operand);
        let n = match &count.kind {
            MirConstKind::Uint(n) => *n as usize,
            MirConstKind::Int(n) => *n as usize,
            other => {
                tracing::debug!("Repeat with non-integer count {:?}, defaulting to 0", other);
                0
            }
        };
        let elem_ty = val.get_type();
        let array_ty = elem_ty.array_type(n.try_into().unwrap_or(0u32));
        let mut array_val = array_ty.const_zero();
        for i in 0..n {
            let inserted = self
                .builder
                .build_insert_value(array_val, val, i as u32, "repeat_insert")
                .expect("insert_value failed for repeat element");
            array_val = inserted.into_array_value();
        }
        array_val.as_basic_value_enum()
    }

    fn type_needs_drop(&self, ty: Ty) -> bool {
        match self.ty_ctx.ty_kind(ty) {
            TyKind::Never
            | TyKind::Unit
            | TyKind::Bool
            | TyKind::Int(_)
            | TyKind::Uint(_)
            | TyKind::Float(_)
            | TyKind::Char => false,
            TyKind::Error => false,
            TyKind::Ref(_, inner, _) | TyKind::RawPtr(inner, _) => self.type_needs_drop(*inner),
            TyKind::Tuple(subst) => {
                let args = self.ty_ctx.substitution_args(*subst);
                args.iter().any(|arg| match arg {
                    glyim_type::GenericArg::Ty(t) => self.type_needs_drop(*t),
                    _ => false,
                })
            }
            TyKind::Array(elem, _) => self.type_needs_drop(*elem),
            TyKind::Adt(_, _) | TyKind::Closure(_, _) | TyKind::FnDef(_, _) => true,
            TyKind::Opaque(_, _) | TyKind::Dynamic(_, _) | TyKind::Slice(_) => true,
            TyKind::Infer(_) | TyKind::Param(_) | TyKind::Bound(_, _) => true,
            TyKind::Projection(_) => true,
            TyKind::FnPtr(_) => false,
            TyKind::String => true,
        }
    }

    fn type_needs_dealloc(&self, ty: Ty) -> bool {
        match self.ty_ctx.ty_kind(ty) {
            TyKind::Ref(_, inner, mutability) if *mutability == Mutability::Mut => {
                !self.ty_ctx.is_copy(*inner)
            }
            TyKind::RawPtr(inner, Mutability::Mut) => !self.ty_ctx.is_copy(*inner),
            _ => false,
        }
    }

    fn lower_drop(
        &mut self,
        place: &Place,
        target: &BasicBlockIdx,
        _cleanup: &Option<BasicBlockIdx>,
    ) -> CompResult<()> {
        let place_ty = self.place_ty(place);
        let target_bb = self.bb_map.get(target).unwrap();

        if self.type_needs_drop(place_ty) {
            let ptr_type = self.context.ptr_type(AddressSpace::default());
            let place_ptr = self.place_ptr(place);
            let i8_ptr = self
                .builder
                .build_bit_cast(place_ptr, ptr_type, "drop_ptr")
                .expect("bitcast for drop failed");
            self.builder
                .build_call(self.drop_fn, &[i8_ptr.into()], "drop_call")
                .map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "Failed to build drop call: {:?}",
                        e
                    ))]
                })?;
        }

        if self.type_needs_dealloc(place_ty) {
            let ptr_type = self.context.ptr_type(AddressSpace::default());
            let place_ptr = self.place_ptr(place);
            let i8_ptr = self
                .builder
                .build_bit_cast(place_ptr, ptr_type, "dealloc_ptr")
                .expect("bitcast for dealloc failed");
            let size = self.llvm_int_type(64).const_int(0, false);
            let align = self.llvm_int_type(64).const_int(0, false);
            self.builder
                .build_call(
                    self.dealloc_fn,
                    &[i8_ptr.into(), size.into(), align.into()],
                    "dealloc_call",
                )
                .map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "Failed to build dealloc call: {:?}",
                        e
                    ))]
                })?;
        }

        self.builder
            .build_unconditional_branch(*target_bb)
            .map_err(|e| {
                vec![GlyimDiagnostic::internal_error(format!(
                    "Failed to build branch after Drop: {:?}",
                    e
                ))]
            })?;
        Ok(())
    }

    fn lower_statement(&mut self, stmt: &Statement) -> CompResult<()> {
        match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                self.set_debug_location(stmt.source_info.span);
                let value = self.lower_rvalue(rvalue);
                let ptr = self.place_ptr(place);
                self.builder.build_store(ptr, value).map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "Failed to build store: {:?}",
                        e
                    ))]
                })?;
                self.clear_debug_location();
            }
            StatementKind::StorageLive(local) => {
                tracing::trace!("StorageLive({})", local.index());
            }
            StatementKind::StorageDead(local) => {
                tracing::trace!("StorageDead({})", local.index());
            }
            StatementKind::Nop => {}
        }
        Ok(())
    }

    #[allow(unused_variables)]
    fn lower_terminator(&mut self, terminator: &Terminator) -> CompResult<()> {
        match &terminator.kind {
            TerminatorKind::Goto { target } => {
                let target_bb = self.bb_map.get(target).unwrap();
                self.builder
                    .build_unconditional_branch(*target_bb)
                    .map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "branch failed: {:?}",
                            e
                        ))]
                    })?;
            }
            TerminatorKind::Return => {
                let return_place = Place::new(LocalIdx::from_raw(0));
                let ret_place_ptr = self.place_ptr(&return_place);
                if let Some(ret_type) = self.function.get_type().get_return_type() {
                    let ret_val = self
                        .builder
                        .build_load(ret_type, ret_place_ptr, "ret_val")
                        .expect("load ret failed");
                    self.builder.build_return(Some(&ret_val)).map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "return failed: {:?}",
                            e
                        ))]
                    })?;
                } else {
                    self.builder.build_return(None).map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "return failed: {:?}",
                            e
                        ))]
                    })?;
                }
            }
            TerminatorKind::Unreachable => {
                self.builder.build_unreachable().map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "unreachable failed: {:?}",
                        e
                    ))]
                })?;
            }
            TerminatorKind::SwitchInt {
                discr,
                switch_ty,
                targets,
            } => {
                let discr_val = self.lower_operand(discr);
                let discr_int = discr_val.into_int_value();
                let n_cases = targets.iter().count();

                if n_cases == 0 {
                    let default_bb = self.bb_map.get(&targets.otherwise()).unwrap();
                    self.builder
                        .build_unconditional_branch(*default_bb)
                        .map_err(|e| vec![GlyimDiagnostic::internal_error(e.to_string())])?;
                    return Ok(());
                }

                if n_cases == 1 {
                    let (value, target_bb_idx) = targets.iter().next().unwrap();
                    let target_bb = self.bb_map.get(&target_bb_idx).unwrap();
                    let default_bb = self.bb_map.get(&targets.otherwise()).unwrap();
                    let value_const = discr_int.get_type().const_int(value as u64, false);
                    let label = if matches!(self.ty_ctx.ty_kind(*switch_ty), TyKind::Bool) {
                        "bool_eq"
                    } else {
                        "switch_eq"
                    };
                    let cmp = self
                        .builder
                        .build_int_compare(inkwell::IntPredicate::EQ, discr_int, value_const, label)
                        .expect("icmp failed");
                    self.builder
                        .build_conditional_branch(cmp, *target_bb, *default_bb)
                        .expect("conditional branch failed");
                    return Ok(());
                }

                if n_cases == 2 {
                    let cases: Vec<_> = targets.iter().collect();
                    let (val0, bb0_idx) = cases[0];
                    let (val1, bb1_idx) = cases[1];
                    let target0 = self.bb_map.get(&bb0_idx).unwrap();
                    let target1 = self.bb_map.get(&bb1_idx).unwrap();
                    let default_bb = self.bb_map.get(&targets.otherwise()).unwrap();

                    let val0_const = discr_int.get_type().const_int(val0 as u64, false);
                    let cmp0 = self
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            discr_int,
                            val0_const,
                            "switch_eq_0",
                        )
                        .expect("icmp0 failed");
                    let next_bb = self
                        .context
                        .append_basic_block(self.function, "switch_next");
                    self.builder
                        .build_conditional_branch(cmp0, *target0, next_bb)
                        .expect("conditional branch0 failed");
                    self.builder.position_at_end(next_bb);
                    let val1_const = discr_int.get_type().const_int(val1 as u64, false);
                    let cmp1 = self
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            discr_int,
                            val1_const,
                            "switch_eq_1",
                        )
                        .expect("icmp1 failed");
                    self.builder
                        .build_conditional_branch(cmp1, *target1, *default_bb)
                        .expect("conditional branch1 failed");
                    return Ok(());
                }

                let default_bb = self.bb_map.get(&targets.otherwise()).unwrap();
                let mut cases = Vec::new();
                for (value, target_bb_idx) in targets.iter() {
                    let value_const = discr_int.get_type().const_int(value as u64, false);
                    let target_bb = self.bb_map.get(&target_bb_idx).unwrap();
                    cases.push((value_const, *target_bb));
                }
                let _switch = self
                    .builder
                    .build_switch(discr_int, *default_bb, &cases)
                    .map_err(|e| vec![GlyimDiagnostic::internal_error(e.to_string())])?;
            }
            TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                cleanup,
            } => {
                self.lower_call(func, args, destination, target, cleanup)?;
            }
            TerminatorKind::Assert {
                cond,
                expected,
                target,
                cleanup,
                msg: _,
            } => {
                let cond_val = self.lower_operand(cond);
                let cond_int = cond_val.into_int_value();
                let expected_val = self
                    .llvm_int_type(1)
                    .const_int(if *expected { 1 } else { 0 }, false);
                let cmp = self
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        cond_int,
                        expected_val,
                        "assert_check",
                    )
                    .expect("assert cmp failed");
                let target_bb = self.bb_map.get(target).unwrap();
                let fail_bb = self
                    .context
                    .append_basic_block(self.function, "assert_fail");
                self.builder
                    .build_conditional_branch(cmp, *target_bb, fail_bb)
                    .expect("assert br failed");
                self.builder.position_at_end(fail_bb);
                let module = self.module;
                let trap_fn_name = "llvm.trap";
                let trap_fn = module.get_function(trap_fn_name).unwrap_or_else(|| {
                    module.add_function(
                        trap_fn_name,
                        self.context.void_type().fn_type(&[], false),
                        None,
                    )
                });
                self.builder
                    .build_call(trap_fn, &[], "trap_call")
                    .expect("trap call failed");
                if let Some(cleanup_bb_idx) = cleanup {
                    let cleanup_bb = self.bb_map.get(cleanup_bb_idx).unwrap();
                    self.builder
                        .build_unconditional_branch(*cleanup_bb)
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "branch to cleanup failed: {:?}",
                                e
                            ))]
                        })?;
                } else {
                    self.builder.build_unreachable().map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "unreachable failed: {:?}",
                            e
                        ))]
                    })?;
                }
            }
            TerminatorKind::Drop {
                place,
                target,
                cleanup,
            } => {
                self.lower_drop(place, target, cleanup)?;
            }
        }
        Ok(())
    }

    fn lower_call(
        &mut self,
        func: &Operand,
        args: &[Operand],
        destination: &Place,
        target: &Option<BasicBlockIdx>,
        cleanup: &Option<BasicBlockIdx>,
    ) -> CompResult<()> {
        let fn_sig = match self.operand_ty(func) {
            ty if matches!(self.ty_ctx.ty_kind(ty), TyKind::FnPtr(_)) => {
                match self.ty_ctx.ty_kind(ty) {
                    TyKind::FnPtr(sig) => sig.clone(),
                    _ => unreachable!(),
                }
            }
            _ => {
                return Err(vec![GlyimDiagnostic::internal_error(
                    "expected function pointer type for call operand",
                )]);
            }
        };

        let layout_computer = FullLayoutComputer::new(self.ty_ctx, self.target_info.clone());
        let fn_abi = layout_computer.fn_abi_of(&fn_sig).map_err(|e| {
            vec![GlyimDiagnostic::internal_error(format!(
                "Layout error: {:?}",
                e
            ))]
        })?;

        let mut param_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = Vec::new();
        let is_sret = matches!(fn_abi.ret.mode, PassMode::Indirect { .. });

        if is_sret {
            param_types.push(self.context.ptr_type(AddressSpace::default()).into());
        }

        for arg_abi in &fn_abi.args {
            let llvm_ty = match arg_abi.mode {
                PassMode::Direct => self.llvm_type_for_ty(arg_abi.ty),
                PassMode::Indirect { .. } => self.context.ptr_type(AddressSpace::default()).into(),
                PassMode::Ignore => continue,
                PassMode::Cast { .. } => {
                    tracing::debug!("Cast PassMode for arg type, using original type");
                    self.llvm_type_for_ty(arg_abi.ty)
                }
                PassMode::HomogeneousAggregate { .. } => {
                    tracing::debug!(
                        "HomogeneousAggregate PassMode for arg type, using original type"
                    );
                    self.llvm_type_for_ty(arg_abi.ty)
                }
                PassMode::Split { .. } => {
                    tracing::debug!("Split PassMode for arg type, using original type");
                    self.llvm_type_for_ty(arg_abi.ty)
                }
            };
            param_types.push(llvm_ty);
        }

        let ret_type: Option<inkwell::types::BasicTypeEnum<'ctx>> = if is_sret {
            None
        } else {
            match fn_abi.ret.mode {
                PassMode::Ignore => None,
                _ => Some(self.llvm_type_for_ty(fn_abi.ret.ty)),
            }
        };

        let metadata_param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
            param_types.iter().map(|ty| (*ty).into()).collect();
        let fn_type = if let Some(ret) = ret_type {
            ret.fn_type(&metadata_param_types, fn_sig.c_variadic)
        } else {
            self.context
                .void_type()
                .fn_type(&metadata_param_types, fn_sig.c_variadic)
        };

        let func_val = self.lower_operand(func).into_pointer_value();

        let mut llvm_args: Vec<inkwell::values::BasicValueEnum<'ctx>> = Vec::new();
        let mut sret_alloca = None;

        if is_sret {
            let sret_llvm_ty = self.llvm_type_for_ty(fn_abi.ret.ty);
            let sret_ptr = self
                .builder
                .build_alloca(sret_llvm_ty, "sret")
                .expect("alloca sret failed");
            llvm_args.push(sret_ptr.as_basic_value_enum());
            sret_alloca = Some(sret_ptr);
        }

        let mut arg_idx = 0;
        for arg_abi in &fn_abi.args {
            if matches!(arg_abi.mode, PassMode::Ignore) {
                continue;
            }
            if arg_idx >= args.len() {
                return Err(vec![GlyimDiagnostic::internal_error(
                    "argument count mismatch",
                )]);
            }
            let arg_op = &args[arg_idx];
            let arg_val = self.lower_operand(arg_op);
            match arg_abi.mode {
                PassMode::Direct => {
                    llvm_args.push(arg_val);
                }
                PassMode::Indirect { .. } => {
                    let ty = arg_val.get_type();
                    let tmp_ptr = self
                        .builder
                        .build_alloca(ty, "arg")
                        .expect("alloca arg failed");
                    self.builder
                        .build_store(tmp_ptr, arg_val)
                        .expect("store arg failed");
                    llvm_args.push(tmp_ptr.as_basic_value_enum());
                }
                PassMode::Ignore => unreachable!(),
                PassMode::Cast { .. } => {
                    tracing::debug!("Cast PassMode for arg, bitcasting through memory");
                    let src_ty = arg_val.get_type();
                    let dest_ty = self.llvm_type_for_ty(arg_abi.ty);
                    if src_ty == dest_ty {
                        llvm_args.push(arg_val);
                    } else {
                        let tmp = self
                            .builder
                            .build_alloca(src_ty, "cast_src")
                            .expect("cast alloca failed");
                        self.builder
                            .build_store(tmp, arg_val)
                            .expect("cast store failed");
                        let cast_ptr = self
                            .builder
                            .build_bit_cast(
                                tmp,
                                self.context.ptr_type(inkwell::AddressSpace::default()),
                                "cast_ptr",
                            )
                            .expect("cast bitcast failed")
                            .into_pointer_value();
                        let cast_val = self
                            .builder
                            .build_load(dest_ty, cast_ptr, "cast_load")
                            .expect("cast load failed");
                        llvm_args.push(cast_val);
                    }
                }
                PassMode::HomogeneousAggregate { .. } => {
                    tracing::debug!("HomogeneousAggregate PassMode for arg, treating as Direct");
                    llvm_args.push(arg_val);
                }
                PassMode::Split { .. } => {
                    tracing::debug!("Split PassMode for arg, treating as Direct");
                    llvm_args.push(arg_val);
                }
            }
            arg_idx += 1;
        }
        let metadata_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            llvm_args.iter().map(|v| (*v).into()).collect();

        let use_invoke = cleanup.is_some();
        let call_result = if use_invoke {
            let normal_bb = if let Some(target_bb) = target {
                *self.bb_map.get(target_bb).expect("target block not found")
            } else {
                return Err(vec![GlyimDiagnostic::internal_error(
                    "invoke requires a target block",
                )]);
            };
            let cleanup_bb = if let Some(cleanup_bb_idx) = cleanup {
                *self
                    .bb_map
                    .get(cleanup_bb_idx)
                    .expect("cleanup block not found")
            } else {
                return Err(vec![GlyimDiagnostic::internal_error(
                    "invoke requires a cleanup block",
                )]);
            };
            self.builder
                .build_indirect_invoke(fn_type, func_val, &llvm_args, normal_bb, cleanup_bb, "call")
                .map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "invoke failed: {:?}",
                        e
                    ))]
                })?
        } else {
            self.builder
                .build_indirect_call(fn_type, func_val, &metadata_args, "call")
                .map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "call failed: {:?}",
                        e
                    ))]
                })?
        };

        if is_sret {
            let sret_attr = self.context.create_enum_attribute(
                inkwell::attributes::Attribute::get_named_enum_kind_id("sret"),
                0,
            );
            call_result.add_attribute(inkwell::attributes::AttributeLoc::Param(1), sret_attr);
        }

        if use_invoke && let Some(target_bb) = target {
            let target_block = self.bb_map.get(target_bb).unwrap();
            self.builder.position_at_end(*target_block);
        }

        if is_sret {
            let sret_ptr = sret_alloca.unwrap();
            let sret_ty = self.llvm_type_for_ty(fn_abi.ret.ty);
            let sret_val = self
                .builder
                .build_load(sret_ty, sret_ptr, "sret_load")
                .expect("load sret failed");
            let dest_ptr = self.place_ptr(destination);
            self.builder
                .build_store(dest_ptr, sret_val)
                .expect("store sret failed");
        } else if !matches!(fn_abi.ret.mode, PassMode::Ignore) {
            let ret_val = match call_result.as_any_value_enum() {
                AnyValueEnum::IntValue(v) => BasicValueEnum::IntValue(v),
                AnyValueEnum::FloatValue(v) => BasicValueEnum::FloatValue(v),
                AnyValueEnum::PointerValue(v) => BasicValueEnum::PointerValue(v),
                AnyValueEnum::StructValue(v) => BasicValueEnum::StructValue(v),
                AnyValueEnum::ArrayValue(v) => BasicValueEnum::ArrayValue(v),
                AnyValueEnum::VectorValue(v) => BasicValueEnum::VectorValue(v),
                AnyValueEnum::ScalableVectorValue(v) => BasicValueEnum::ScalableVectorValue(v),
                _ => {
                    return Err(vec![GlyimDiagnostic::internal_error(
                        "unexpected call return kind",
                    )]);
                }
            };
            let dest_ptr = self.place_ptr(destination);
            self.builder
                .build_store(dest_ptr, ret_val)
                .expect("store ret failed");
        }

        if !use_invoke {
            if let Some(target_bb) = target {
                let target_block = self.bb_map.get(target_bb).expect("target block not found");
                self.builder
                    .build_unconditional_branch(*target_block)
                    .expect("branch failed");
            } else {
                self.builder
                    .build_unreachable()
                    .expect("unreachable failed");
            }
        }
        Ok(())
    }
}

pub(crate) fn lower_body<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    body: &Body,
    target_info: TargetInfo,
    ty_ctx: &TyCtx,
    debug_info: bool,
    source_map: HashMap<FileId, (String, String)>,
) -> CompResult<()> {
    let fn_name = format!(
        "func_{}_{}",
        body.owner.krate.to_raw(),
        body.owner.local_id.to_raw()
    );

    let ret_llvm_ty = llvm_type_for_ty(ty_ctx, &target_info, context, body.return_ty);
    let void_type = context.void_type();

    let mut param_types = Vec::new();
    for i in 1..=body.arg_count {
        let local_idx = LocalIdx::from_raw(i as u32);
        if let Some(local_decl) = body.locals.get(local_idx) {
            let param_ty = llvm_type_for_ty(ty_ctx, &target_info, context, local_decl.ty);
            param_types.push(param_ty.into());
        }
    }

    let fn_type = if matches!(ty_ctx.ty_kind(body.return_ty), TyKind::Never | TyKind::Unit)
        || body.return_ty == Ty::NEVER
        || body.return_ty == Ty::UNIT
    {
        void_type.fn_type(&param_types, false)
    } else {
        ret_llvm_ty.fn_type(&param_types, false)
    };

    let function = module.add_function(&fn_name, fn_type, None);

    let mut debug_ctx = if debug_info {
        Some(DebugInfoCtx::new(context, module, source_map, true))
    } else {
        None
    };

    // Attach subprogram if debug info is enabled
    if let Some(ref mut di) = debug_ctx {
        di.set_function(context, &function, &fn_name, FileId::from_raw(0), 1);
    }

    let entry_block = context.append_basic_block(function, "entry");
    let builder = context.create_builder();
    builder.position_at_end(entry_block);

    let mut bb_map: HashMap<BasicBlockIdx, inkwell::basic_block::BasicBlock<'ctx>> = HashMap::new();
    bb_map.insert(BasicBlockIdx::from_raw(0), entry_block);

    for (bb_idx, _bb_data) in body.basic_blocks.iter_enumerated() {
        if bb_idx != BasicBlockIdx::from_raw(0) {
            let bb_name = format!("bb_{}", bb_idx.index());
            let llvm_bb = context.append_basic_block(function, &bb_name);
            bb_map.insert(bb_idx, llvm_bb);
        }
    }

    let num_locals = body.locals.len();
    let mut locals: IndexVec<LocalIdx, Option<PointerValue<'ctx>>> =
        IndexVec::with_capacity(num_locals);
    for _ in 0..num_locals {
        locals.push(None);
    }

    let ptr_type = context.ptr_type(AddressSpace::default());
    let i64_type = context
        .custom_width_int_type(NonZeroU32::new(64).unwrap())
        .unwrap();

    let drop_fn_type = void_type.fn_type(&[ptr_type.into()], false);
    let drop_fn = module
        .get_function("glyim_drop_in_place")
        .unwrap_or_else(|| module.add_function("glyim_drop_in_place", drop_fn_type, None));

    let dealloc_fn_type =
        void_type.fn_type(&[ptr_type.into(), i64_type.into(), i64_type.into()], false);
    let dealloc_fn = module
        .get_function("glyim_dealloc")
        .unwrap_or_else(|| module.add_function("glyim_dealloc", dealloc_fn_type, None));

    let has_cleanup = body.basic_blocks.iter().any(|bb| bb.is_cleanup);
    let personality_fn = if has_cleanup {
        let personality_fn_type = context.void_type().fn_type(&[], false);
        let personality_fn = module.add_function("__glyim_personality", personality_fn_type, None);
        function.set_personality_function(personality_fn);
        Some(personality_fn)
    } else {
        None
    };

    let mut lowering_ctx = LoweringCtx {
        context,
        builder,
        function,
        module,
        drop_fn,
        dealloc_fn,
        body,
        target_info,
        ty_ctx,
        locals,
        bb_map,
        personality_fn,
        debug_ctx,
    };

    for (local_idx, _local_decl) in body.locals.iter_enumerated() {
        lowering_ctx.alloc_local(local_idx);
    }

    for (bb_idx, bb_data) in body.basic_blocks.iter_enumerated() {
        let llvm_bb = lowering_ctx.bb_map.get(&bb_idx).unwrap();
        lowering_ctx.builder.position_at_end(*llvm_bb);
        if bb_data.is_cleanup {
            lowering_ctx.emit_landingpad()?;
        }
        for stmt in &bb_data.statements {
            lowering_ctx.lower_statement(stmt)?;
        }
        lowering_ctx.lower_terminator(&bb_data.terminator)?;
    }

    if let Some(di) = lowering_ctx.debug_ctx {
        di.finalize();
    }
    Ok(())
}
