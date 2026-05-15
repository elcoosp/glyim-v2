use glyim_codegen::CodegenBackend;
use glyim_core::arena::IndexVec;
use glyim_core::primitives::*;
use glyim_diag::{CompResult, GlyimDiagnostic};
use glyim_mir::{
    AggregateKind, BasicBlockIdx, Body, LocalIdx, MirConst, MirConstKind, Operand, Place, Rvalue,
    Statement, StatementKind, Terminator, TerminatorKind,
};
use glyim_type::Ty;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::{InitializationConfig, Target, TargetTriple};
use inkwell::types::BasicType;
use inkwell::values::{BasicValue, BasicValueEnum, IntValue, PointerValue};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;

pub struct LlvmBackend {
    context: Context,
    target_triple: String,
}

impl Default for LlvmBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl LlvmBackend {
    pub fn new() -> Self {
        Target::initialize_all(&InitializationConfig::default());
        Self {
            context: Context::create(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
        }
    }

    pub fn with_target(target_triple: impl Into<String>) -> Self {
        Target::initialize_all(&InitializationConfig::default());
        Self {
            context: Context::create(),
            target_triple: target_triple.into(),
        }
    }
}

impl CodegenBackend for LlvmBackend {
    fn name(&self) -> &'static str {
        "llvm"
    }

    fn generate(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<()> {
        let context = &self.context;
        let module = context.create_module("glyim_module");
        let triple = TargetTriple::create(&self.target_triple);
        module.set_triple(&triple);

        let target = Target::from_triple(&triple).map_err(|e| {
            vec![GlyimDiagnostic::internal_error(format!(
                "Target error: {}",
                e
            ))]
        })?;

        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                inkwell::OptimizationLevel::Default,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .ok_or_else(|| {
                vec![GlyimDiagnostic::internal_error(
                    "Failed to create target machine",
                )]
            })?;

        for body in bodies.iter() {
            self.lower_body(context, &module, body)?;
        }

        target_machine
            .write_to_file(&module, inkwell::targets::FileType::Object, output)
            .map_err(|e| {
                vec![GlyimDiagnostic::internal_error(format!(
                    "Failed to write object file: {:?}",
                    e
                ))]
            })?;

        Ok(())
    }

    fn generate_function(&self, body: &Arc<Body>) -> CompResult<Vec<u8>> {
        let context = &self.context;
        let module = context.create_module("glyim_func");
        let triple = TargetTriple::create(&self.target_triple);
        module.set_triple(&triple);

        self.lower_body(context, &module, body)?;

        let target = Target::from_triple(&triple).map_err(|e| {
            vec![GlyimDiagnostic::internal_error(format!(
                "Target error: {}",
                e
            ))]
        })?;

        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                inkwell::OptimizationLevel::Default,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .ok_or_else(|| {
                vec![GlyimDiagnostic::internal_error(
                    "Failed to create target machine",
                )]
            })?;

        target_machine
            .write_to_memory_buffer(&module, inkwell::targets::FileType::Object)
            .map(|buf| buf.as_slice().to_vec())
            .map_err(|e| {
                vec![GlyimDiagnostic::internal_error(format!(
                    "Failed to generate object code: {:?}",
                    e
                ))]
            })
    }
}

struct LoweringCtx<'ctx, 'a> {
    context: &'ctx Context,
    builder: Builder<'ctx>,
    _function: inkwell::values::FunctionValue<'ctx>,
    body: &'a Body,
    _target_info: TargetInfo,
    locals: IndexVec<LocalIdx, Option<PointerValue<'ctx>>>,
    bb_map: HashMap<BasicBlockIdx, inkwell::basic_block::BasicBlock<'ctx>>,
}

impl<'ctx, 'a> LoweringCtx<'ctx, 'a> {
    fn llvm_int_type(&self, bits: u32) -> inkwell::types::IntType<'ctx> {
        let non_zero = NonZeroU32::new(bits).unwrap_or(NonZeroU32::new(64).unwrap());
        self.context.custom_width_int_type(non_zero).unwrap()
    }

    fn llvm_type_for_ty(&self, ty: Ty) -> inkwell::types::BasicTypeEnum<'ctx> {
        match ty.to_raw() {
            0 => {
                tracing::warn!("STUB: error Ty maps to i64");
                self.llvm_int_type(64).into()
            }
            1 => self.context.struct_type(&[], false).into(),
            2 => self.context.struct_type(&[], false).into(),
            3 => self.llvm_int_type(1).into(),
            _ => {
                tracing::warn!("STUB: unknown Ty {} maps to i64", ty.to_raw());
                self.llvm_int_type(64).into()
            }
        }
    }

    fn alloc_local(&mut self, local: LocalIdx) {
        let ty = self.body.locals[local].ty;
        let llvm_ty = self.llvm_type_for_ty(ty);
        let name = format!("local_{}", local.index());
        let alloca = self
            .builder
            .build_alloca(llvm_ty, &name)
            .expect("alloca failed");
        self.locals[local] = Some(alloca);
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
            MirConstKind::String(_) => {
                tracing::warn!("STUB: string constant lowering");
                self.llvm_int_type(64).const_zero().into()
            }
            MirConstKind::Fn(_, _) => {
                tracing::warn!("STUB: fn constant lowering");
                self.llvm_int_type(64).const_zero().into()
            }
            MirConstKind::ConstRef(_, _) => {
                tracing::warn!("STUB: const ref lowering");
                self.llvm_int_type(64).const_zero().into()
            }
            MirConstKind::Error => {
                tracing::warn!("STUB: error constant lowered as i64 zero");
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
        for elem in place.projection.iter() {
            match elem {
                glyim_mir::ProjectionElem::Deref => {
                    let current_ty = self.body.locals[place.local].ty;
                    let llvm_ty = self.llvm_type_for_ty(current_ty);
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, ptr, "deref_load")
                        .expect("deref load failed");
                    ptr = loaded.into_pointer_value();
                }
                glyim_mir::ProjectionElem::Field(idx) => {
                    let field_idx = idx.to_raw() as u64;
                    let i32_type = self.llvm_int_type(32);
                    let zero = i32_type.const_zero();
                    let field_index = i32_type.const_int(field_idx, false);
                    let current_ty = self.body.locals[place.local].ty;
                    let llvm_ty = self.llvm_type_for_ty(current_ty);
                    ptr = unsafe {
                        self.builder
                            .build_in_bounds_gep(llvm_ty, ptr, &[zero, field_index], "field_gep")
                            .expect("field gep failed")
                    };
                }
                glyim_mir::ProjectionElem::Index(local_idx) => {
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
                    let current_ty = self.body.locals[place.local].ty;
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
                }
                glyim_mir::ProjectionElem::Downcast(_) => {
                    tracing::debug!("Downcast projection - treating as no-op on ptr");
                }
            }
        }
        ptr
    }

    fn place_ty(&self, place: &Place) -> Ty {
        self.body.locals[place.local].ty
    }

    fn lower_rvalue(&self, rvalue: &Rvalue) -> BasicValueEnum<'ctx> {
        match rvalue {
            Rvalue::Use(operand) => self.lower_operand(operand),
            Rvalue::Ref(place, _borrow_kind) => {
                let ptr = self.place_ptr(place);
                ptr.as_basic_value_enum()
            }
            Rvalue::BinaryOp(op, operands) => {
                let lhs = self.lower_operand(&operands.0).into_int_value();
                let rhs = self.lower_operand(&operands.1).into_int_value();
                self.lower_binary_op(*op, lhs, rhs)
            }
            Rvalue::UnaryOp(op, operand) => {
                let val = self.lower_operand(operand).into_int_value();
                self.lower_unary_op(*op, val)
            }
            Rvalue::Aggregate(kind, operands) => self.lower_aggregate(kind, operands),
            Rvalue::Discriminant(place) => self.lower_discriminant(place),
            Rvalue::Len(place) => self.lower_len(place),
            Rvalue::Cast(cast_kind, operand, _ty) => self.lower_cast(*cast_kind, operand),
            Rvalue::Repeat(operand, count) => self.lower_repeat(operand, count),
        }
    }

    fn lower_aggregate(&self, kind: &AggregateKind, operands: &[Operand]) -> BasicValueEnum<'ctx> {
        match kind {
            AggregateKind::Tuple => {
                let field_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = operands
                    .iter()
                    .map(|op| self.lower_operand(op).get_type())
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
            AggregateKind::Array(_elem_ty) => {
                let field_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = operands
                    .iter()
                    .map(|op| self.lower_operand(op).get_type())
                    .collect();
                let elem_ty = if field_types.is_empty() {
                    self.llvm_int_type(8).into()
                } else {
                    field_types[0]
                };
                let array_ty = elem_ty.array_type(operands.len() as u32);
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
                tracing::warn!("STUB: ADT aggregate lowering treats as tuple-like struct");
                let field_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = operands
                    .iter()
                    .map(|op| self.lower_operand(op).get_type())
                    .collect();
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
                tracing::warn!("STUB: Closure aggregate lowering treats as tuple-like struct");
                let field_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = operands
                    .iter()
                    .map(|op| self.lower_operand(op).get_type())
                    .collect();
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

        match place_ty.to_raw() {
            3 => {
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
                let i32_ty = self.llvm_int_type(32);
                let discr_ptr = unsafe {
                    self.builder
                        .build_in_bounds_gep(
                            i32_ty,
                            ptr,
                            &[i32_ty.const_zero(), i32_ty.const_zero()],
                            "discr_gep",
                        )
                        .expect("discr gep failed")
                };
                self.builder
                    .build_load(i32_ty, discr_ptr, "discr_load")
                    .expect("discr load failed")
            }
        }
    }

    fn lower_len(&self, place: &Place) -> BasicValueEnum<'ctx> {
        let _place_ty = self.place_ty(place);
        let i64_ty = self.llvm_int_type(64);
        i64_ty.const_zero().into()
    }

    fn lower_binary_op(
        &self,
        op: BinOp,
        lhs: IntValue<'ctx>,
        rhs: IntValue<'ctx>,
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
            BinOp::Div => self
                .builder
                .build_int_signed_div(lhs, rhs, "sdiv")
                .expect("sdiv failed")
                .into(),
            BinOp::Rem => self
                .builder
                .build_int_signed_rem(lhs, rhs, "srem")
                .expect("srem failed")
                .into(),
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
            BinOp::Lt => self
                .builder
                .build_int_compare(inkwell::IntPredicate::SLT, lhs, rhs, "lt")
                .expect("lt failed")
                .into(),
            BinOp::Gt => self
                .builder
                .build_int_compare(inkwell::IntPredicate::SGT, lhs, rhs, "gt")
                .expect("gt failed")
                .into(),
            BinOp::LtEq => self
                .builder
                .build_int_compare(inkwell::IntPredicate::SLE, lhs, rhs, "le")
                .expect("le failed")
                .into(),
            BinOp::GtEq => self
                .builder
                .build_int_compare(inkwell::IntPredicate::SGE, lhs, rhs, "ge")
                .expect("ge failed")
                .into(),
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
                .build_right_shift(lhs, rhs, true, "shr")
                .expect("shr failed")
                .into(),
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
                tracing::warn!("STUB: UnaryOp::Deref should not appear here");
                val.into()
            }
        }
    }

    fn lower_cast(&self, kind: glyim_mir::CastKind, operand: &Operand) -> BasicValueEnum<'ctx> {
        let val = self.lower_operand(operand);
        match kind {
            glyim_mir::CastKind::IntToInt => {
                let int_val = val.into_int_value();
                let src_bits = int_val.get_type().get_bit_width();
                let dest_type = self.llvm_int_type(src_bits);
                if src_bits < 64 {
                    self.builder
                        .build_int_s_extend(int_val, dest_type, "int_to_int")
                        .expect("int_to_int ext failed")
                        .into()
                } else {
                    self.builder
                        .build_int_truncate(int_val, dest_type, "int_to_int")
                        .expect("int_to_int trunc failed")
                        .into()
                }
            }
            glyim_mir::CastKind::FloatToInt => {
                tracing::warn!("STUB: FloatToInt cast");
                val
            }
            glyim_mir::CastKind::IntToFloat => {
                tracing::warn!("STUB: IntToFloat cast");
                val
            }
            glyim_mir::CastKind::PtrToPtr => val,
            glyim_mir::CastKind::FnPtrToPtr => val,
        }
    }

    fn lower_repeat(&self, operand: &Operand, count: &MirConst) -> BasicValueEnum<'ctx> {
        let val = self.lower_operand(operand);
        let n = match &count.kind {
            MirConstKind::Uint(n) => *n as usize,
            MirConstKind::Int(n) => *n as usize,
            _ => {
                tracing::warn!("STUB: Repeat with non-integer count, defaulting to 0");
                0
            }
        };
        let elem_ty = val.get_type();
        let array_ty = elem_ty.array_type(n as u32);
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
}

impl LlvmBackend {
    fn lower_body<'ctx>(
        &'ctx self,
        context: &'ctx Context,
        module: &Module<'ctx>,
        body: &Body,
    ) -> CompResult<()> {
        let fn_name = format!(
            "func_{}_{}",
            body.owner.krate.to_raw(),
            body.owner.local_id.to_raw()
        );

        let void_type = context.void_type();
        let fn_type = void_type.fn_type(&[], false);
        let function = module.add_function(&fn_name, fn_type, None);
        let entry_block = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry_block);

        let mut bb_map: HashMap<BasicBlockIdx, inkwell::basic_block::BasicBlock<'ctx>> =
            HashMap::new();
        bb_map.insert(BasicBlockIdx::from_raw(0), entry_block);

        for (bb_idx, _bb_data) in body.basic_blocks.iter_enumerated() {
            if bb_idx != BasicBlockIdx::from_raw(0) {
                let bb_name = format!("bb_{}", bb_idx.index());
                let llvm_bb = context.append_basic_block(function, &bb_name);
                bb_map.insert(bb_idx, llvm_bb);
            }
        }

        let target_info = TargetInfo::default();

        let num_locals = body.locals.len();
        let mut locals: IndexVec<LocalIdx, Option<PointerValue<'ctx>>> =
            IndexVec::with_capacity(num_locals);
        for _ in 0..num_locals {
            locals.push(None);
        }

        let mut lowering_ctx = LoweringCtx {
            context,
            builder,
            _function: function,
            body,
            _target_info: target_info,
            locals,
            bb_map,
        };

        for (local_idx, _local_decl) in body.locals.iter_enumerated() {
            lowering_ctx.alloc_local(local_idx);
        }

        for (bb_idx, bb_data) in body.basic_blocks.iter_enumerated() {
            let llvm_bb = lowering_ctx.bb_map.get(&bb_idx).unwrap();
            lowering_ctx.builder.position_at_end(*llvm_bb);

            for stmt in &bb_data.statements {
                lowering_ctx.lower_statement(stmt)?;
            }

            lowering_ctx.lower_terminator(&bb_data.terminator)?;
        }

        Ok(())
    }
}

impl<'ctx, 'a> LoweringCtx<'ctx, 'a> {
    fn lower_statement(&mut self, stmt: &Statement) -> CompResult<()> {
        match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                let value = self.lower_rvalue(rvalue);
                let ptr = self.place_ptr(place);
                self.builder.build_store(ptr, value).map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "Failed to build store: {:?}",
                        e
                    ))]
                })?;
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

    fn lower_terminator(&mut self, terminator: &Terminator) -> CompResult<()> {
        match &terminator.kind {
            TerminatorKind::Goto { target } => {
                let target_bb = self.bb_map.get(target).unwrap();
                self.builder
                    .build_unconditional_branch(*target_bb)
                    .map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "Failed to build unconditional branch: {:?}",
                            e
                        ))]
                    })?;
            }
            TerminatorKind::Return => {
                self.builder.build_return(None).map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "Failed to build return: {:?}",
                        e
                    ))]
                })?;
            }
            TerminatorKind::Unreachable => {
                self.builder.build_unreachable().map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "Failed to build unreachable: {:?}",
                        e
                    ))]
                })?;
            }
            TerminatorKind::SwitchInt {
                discr,
                switch_ty: _,
                targets,
            } => {
                let discr_val = self.lower_operand(discr).into_int_value();
                let otherwise_bb = self.bb_map.get(&targets.otherwise()).unwrap();

                let mut cases = Vec::new();
                for (value, target_bb) in targets.iter() {
                    let target_block = self.bb_map.get(&target_bb).unwrap();
                    let case_val = discr_val.get_type().const_int(value as u64, false);
                    cases.push((case_val, *target_block));
                }

                self.builder
                    .build_switch(discr_val, *otherwise_bb, &cases)
                    .map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "Failed to build switch: {:?}",
                            e
                        ))]
                    })?;
            }
            TerminatorKind::Call { .. } => {
                tracing::warn!("STUB: Call terminator not yet implemented");
            }
            TerminatorKind::Assert { .. } => {
                tracing::warn!("STUB: Assert terminator not yet implemented");
            }
            TerminatorKind::Drop {
                place: _,
                target,
                cleanup: _,
            } => {
                tracing::warn!("STUB: Drop terminator not yet implemented, jumping to target");
                let target_bb = self.bb_map.get(target).unwrap();
                self.builder
                    .build_unconditional_branch(*target_bb)
                    .map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "Failed to build branch for Drop: {:?}",
                            e
                        ))]
                    })?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
