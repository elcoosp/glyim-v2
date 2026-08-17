use crate::abi::FullLayoutComputer;
use crate::debug::DebugInfoCtx;
use crate::types::llvm_type_for_ty;
use glyim_core::TargetInfo;
use glyim_core::arena::IndexVec;
use glyim_core::primitives::*;
use glyim_diag::{CompResult, GlyimDiagnostic};
use glyim_layout::{FieldsShape, LayoutComputer, PassMode, Size, TagEncoding, VariantsShape};
use glyim_mir::VariantIdx;
use glyim_mir::{
    AggregateKind, BasicBlockIdx, Body, CastKind, LocalIdx, MirConst, MirConstKind, Operand, Place,
    ProjectionElem, Rvalue, Statement, StatementKind, Terminator, TerminatorKind,
};
use glyim_span::HygieneCtx;
use glyim_span::{FileId, Span};
use glyim_type::{ConstKind, FieldIdx, Substitution, Ty, TyCtx, TyKind, FnSig};
use glyim_core::primitives::{Abi, Safety};
use inkwell::AddressSpace;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::BasicType;
use inkwell::values::{
    AnyValue, AnyValueEnum, BasicMetadataValueEnum, BasicValue, BasicValueEnum, PointerValue,
};
use std::collections::HashMap;
use std::num::NonZeroU32;

/// Plan §19.1 / §19.2: personality-function selection is a proper three-way
/// choice driven by target ABI, instead of the previous implicit
/// "Windows or Unix, nothing else" binary. `None` means the target/profile has
/// no unwinding support, in which case `Call` terminators with a cleanup target
/// must lower to a plain `call` (no `invoke`/landingpad) — see `lower_call`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Personality {
    /// Itanium C++ ABI personality (`__gcc_personality_v0` / `__gxx_personality_v0`)
    /// used on Linux/glibc, macOS, and other Unix-like targets.
    Itanium,
    /// Microsoft SEH personality (`__CxxFrameHandler3`) used on Windows targets.
    /// Requires funclet-based landingpads (`cleanuppad`/`catchpad`) rather than
    /// the Itanium `landingpad`/`resume` pair (deferred — see §19.1).
    Seh,
    /// No unwinding support (e.g. `panic = "abort"` or a bare-metal target with
    /// no unwinder). No personality function is emitted.
    None,
}

/// Select the personality kind for the given target, given whether the body
/// has any cleanup (unwind) blocks. Without cleanup blocks there is nothing to
/// unwind into, so we emit no personality function regardless of target.
pub fn select_personality(target: &TargetInfo, has_cleanup: bool) -> Personality {
    if !has_cleanup {
        return Personality::None;
    }
    match target.abi {
        TargetAbi::X86_64Windows | TargetAbi::AArch64Windows => Personality::Seh,
        _ => Personality::Itanium,
    }
}

#[allow(unused_imports)]
fn local_ty(body: &Body, local: LocalIdx) -> Ty {
    body.locals[local].ty
}

struct LoweringCtx<'ctx, 'a> {
    context: &'ctx Context,
    builder: Builder<'ctx>,
    function: inkwell::values::FunctionValue<'ctx>,
    module: &'a Module<'ctx>,
    body: &'a Body,
    target_info: TargetInfo,
    ty_ctx: &'a TyCtx,
    locals: IndexVec<LocalIdx, Option<PointerValue<'ctx>>>,
    bb_map: HashMap<BasicBlockIdx, inkwell::basic_block::BasicBlock<'ctx>>,
    _personality_fn: Option<inkwell::values::FunctionValue<'ctx>>,
    debug_ctx: Option<DebugInfoCtx<'ctx>>,
    /// The landingpad result value for the cleanup block currently being
    /// lowered, if any. Set at the top of `lower_body`'s block loop (via
    /// `emit_landingpad`) and consulted by `TerminatorKind::Unreachable` to
    /// decide whether to emit `resume` instead of `unreachable`.
    current_landingpad: Option<BasicValueEnum<'ctx>>,
}
impl<'ctx, 'a> LoweringCtx<'ctx, 'a> {
    fn llvm_int_type(&self, bits: u32) -> inkwell::types::IntType<'ctx> {
        let non_zero = NonZeroU32::new(bits).unwrap_or_else(|| NonZeroU32::new(64).unwrap());
        self.context.custom_width_int_type(non_zero).unwrap()
    }

    /// Build an LLVM function type from a Glyim `FnSig`, applying ABI rules.
    fn llvm_fn_type_from_sig(&self, sig: &glyim_type::FnSig) -> inkwell::types::FunctionType<'ctx> {
        let layout_computer = FullLayoutComputer::new(self.ty_ctx, self.target_info.clone());
        let fn_abi = layout_computer.fn_abi_of(sig).unwrap();

        let is_sret = matches!(fn_abi.ret.mode, PassMode::Indirect { .. });
        let mut param_types = Vec::new();
        if is_sret {
            param_types.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        for arg_abi in &fn_abi.args {
            let llvm_ty = match arg_abi.mode {
                PassMode::Direct => self.llvm_type_for_ty(arg_abi.ty),
                PassMode::Indirect { .. } => self.context.ptr_type(AddressSpace::default()).into(),
                PassMode::Ignore => continue,
                _ => self.llvm_type_for_ty(arg_abi.ty),
            };
            param_types.push(llvm_ty);
        }
        let ret_type = if is_sret {
            None
        } else {
            match fn_abi.ret.mode {
                PassMode::Ignore => None,
                _ => Some(self.llvm_type_for_ty(fn_abi.ret.ty)),
            }
        };
        let metadata_param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
            param_types.iter().map(|t| (*t).into()).collect();
        if let Some(ret) = ret_type {
            ret.fn_type(&metadata_param_types, sig.c_variadic)
        } else {
            self.context
                .void_type()
                .fn_type(&metadata_param_types, sig.c_variadic)
        }
    }

    fn llvm_type_for_ty(&self, ty: Ty) -> inkwell::types::BasicTypeEnum<'ctx> {
        match llvm_type_for_ty(self.ty_ctx, &self.target_info, self.context, ty) {
            Ok(t) => t,
            Err(e) => panic!("Codegen error: {:?}", e),
        }
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

    fn alloc_local(&mut self, local: LocalIdx) {
        let ty = local_ty(self.body, local);
        let llvm_ty = self.llvm_type_for_ty(ty);
        let name = format!("local_{}", local.index());
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
            // Plan §19.3: enforce the *real* computed alignment at every alloca
            // site unconditionally (not only above a 16-byte threshold). The
            // opaque sized type cannot carry alignment > 16 through its type
            // alone, and even <= 16 types must get their true alignment so the
            // backend never silently under-aligns (e.g. `#[repr(align(64))]`).
            let layout_computer = FullLayoutComputer::new(self.ty_ctx, self.target_info.clone());
            if let Ok(layout) = layout_computer.layout_of(ty) {
                let align = layout.align.0.max(1) as u32;
                if let Some(alloca_inst) = alloca.as_instruction_value() {
                    let _ = alloca_inst.set_alignment(align);
                }
            }
            self.locals[local] = Some(alloca);
        }
    }

    fn get_local_ptr(&self, local: LocalIdx) -> PointerValue<'ctx> {
        self.locals[local].unwrap_or_else(|| panic!("local {} not allocated", local.index()))
    }

    fn lower_operand(&self, operand: &Operand) -> CompResult<BasicValueEnum<'ctx>> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => {
                let ptr = self.place_ptr(place)?;
                let ty = self.place_ty(place);
                let llvm_ty = self.llvm_type_for_ty(ty);
                Ok(self
                    .builder
                    .build_load(llvm_ty, ptr, "load")
                    .expect("load failed"))
            }
            Operand::Constant(c) => self.lower_const(c),
        }
    }

    fn lower_const(&self, c: &MirConst) -> CompResult<BasicValueEnum<'ctx>> {
        match &c.kind {
            MirConstKind::Int(v) => {
                let ty = self.llvm_type_for_ty(c.ty);
                let int_ty = ty.into_int_type();
                Ok(int_ty.const_int(*v as u64, true).into())
            }
            MirConstKind::Uint(v) => {
                let ty = self.llvm_type_for_ty(c.ty);
                let int_ty = ty.into_int_type();
                Ok(int_ty.const_int(*v as u64, false).into())
            }
            MirConstKind::Bool(b) => Ok(self
                .llvm_int_type(1)
                .const_int(if *b { 1 } else { 0 }, false)
                .into()),
            MirConstKind::FloatBits(bits) => {
                let ty = self.llvm_type_for_ty(c.ty);
                let float_ty = ty.into_float_type();
                Ok(float_ty.const_float(f64::from_bits(*bits)).into())
            }
            MirConstKind::Char(ch) => {
                Ok(self.llvm_int_type(32).const_int(*ch as u64, false).into())
            }
            MirConstKind::Unit => {
                let unit_ty = self.context.struct_type(&[], false);
                Ok(unit_ty.const_zero().as_basic_value_enum())
            }
            MirConstKind::String(name) => {
                let str_content = self.ty_ctx.name_str(*name);
                let module = self.module;
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                str_content.hash(&mut hasher);
                let hash = hasher.finish();
                let global_name = format!("__glyim_str_{:016x}", hash);
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
                let i64_type = self.context.i64_type();
                let struct_ty = self
                    .context
                    .struct_type(&[ptr_type.into(), i64_type.into()], false);
                let str_ptr = self
                    .builder
                    .build_bit_cast(global.as_pointer_value(), ptr_type, "str_ptr")
                    .expect("bitcast for string constant failed")
                    .into_pointer_value();
                let len_val = i64_type.const_int(str_content.len() as u64, false);
                let agg = struct_ty.const_zero();
                let inserted_ptr = self
                    .builder
                    .build_insert_value(agg, str_ptr, 0, "str_ptr_insert")
                    .expect("insert ptr failed");
                let inserted_ptr = match inserted_ptr {
                    inkwell::values::AggregateValueEnum::StructValue(s) => s,
                    _ => unreachable!(),
                };
                let inserted_len = self
                    .builder
                    .build_insert_value(inserted_ptr, len_val, 1, "str_len_insert")
                    .expect("insert len failed");
                match inserted_len {
                    inkwell::values::AggregateValueEnum::StructValue(s) => {
                        Ok(s.as_basic_value_enum())
                    }
                    _ => unreachable!(),
                }
            }
            MirConstKind::Fn(fn_def_id, _substs) => {
                let fn_name = format!("__glyim_fn_{}", fn_def_id.to_raw());
                let module = self.module;
                let callee = module.get_function(&fn_name).unwrap_or_else(|| {
                    let fn_type = if let Some(sig) = self.ty_ctx.fn_sig(*fn_def_id) {
                        self.llvm_fn_type_from_sig(sig)
                    } else {
                        self.context.void_type().fn_type(&[], false)
                    };
                    module.add_function(&fn_name, fn_type, None)
                });
                Ok(callee
                    .as_global_value()
                    .as_pointer_value()
                    .as_basic_value_enum())
            }
            MirConstKind::ConstRef(const_def_id, _substs) => {
                let global_name = format!("__glyim_const_{}", const_def_id.to_raw());
                let module = self.module;
                let global = module.get_global(&global_name).unwrap_or_else(|| {
                    let llvm_ty = self.llvm_type_for_ty(c.ty);
                    let global = module.add_global(
                        llvm_ty,
                        Some(inkwell::AddressSpace::default()),
                        &global_name,
                    );
                    global.set_initializer(&llvm_ty.const_zero());
                    global.set_constant(true);
                    global.set_linkage(inkwell::module::Linkage::Internal);
                    // Plan §19.3: enforce the real computed alignment on the
                    // global unconditionally (not only above 16 bytes).
                    let layout_computer =
                        FullLayoutComputer::new(self.ty_ctx, self.target_info.clone());
                    if let Ok(layout) = layout_computer.layout_of(c.ty) {
                        global.set_alignment(layout.align.0.max(1) as u32);
                    }
                    global
                });
                let llvm_ty = self.llvm_type_for_ty(c.ty);
                Ok(self
                    .builder
                    .build_load(llvm_ty, global.as_pointer_value(), "const_ref_load")
                    .expect("const ref load failed"))
            }
            MirConstKind::Aggregate(elems) => {
                // Emit a constant aggregate (tuple/array/struct) by recursively
                // lowering each constant element and inserting it into a zero
                // aggregate at its index (plan §15.3). `build_insert_value`
                // works uniformly for both struct and array LLVM types. Any
                // element that fails to lower falls back to a zero constant so
                // callers never observe an error.
                let llvm_ty = self.llvm_type_for_ty(c.ty);
                let zero = llvm_ty.const_zero();
                let agg: inkwell::values::AggregateValueEnum = match llvm_ty {
                    inkwell::types::BasicTypeEnum::StructType(st) => st.const_zero().into(),
                    inkwell::types::BasicTypeEnum::ArrayType(at) => at.const_zero().into(),
                    // Scalars/others: nothing to aggregate; return the zero.
                    _ => return Ok(zero.as_basic_value_enum()),
                };
                let mut cur = agg;
                for (i, e) in elems.iter().enumerate() {
                    let val = self
                        .lower_const(e)
                        .unwrap_or_else(|_| zero.as_basic_value_enum());
                    cur = self
                        .builder
                        .build_insert_value(cur, val, i as u32, "agg_elem")
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "aggregate constant insert failed: {:?}",
                                e
                            ))]
                        })?;
                }
                Ok(match cur {
                    inkwell::values::AggregateValueEnum::StructValue(s) => s.as_basic_value_enum(),
                    inkwell::values::AggregateValueEnum::ArrayValue(a) => a.as_basic_value_enum(),
                })
            }
            MirConstKind::Error => Err(vec![GlyimDiagnostic::internal_error(
                "internal compiler error: MirConstKind::Error reached codegen",
            )]),
        }
    }

    fn place_ptr(&self, place: &Place) -> CompResult<PointerValue<'ctx>> {
        let base = self.get_local_ptr(place.local);
        if place.projection.is_empty() {
            return Ok(base);
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
                        _ => {
                            return Err(vec![GlyimDiagnostic::internal_error(format!(
                                "internal compiler error: Deref on non-pointer type {:?} – MIR is invalid",
                                current_ty
                            ))]);
                        }
                    };
                }
                ProjectionElem::Field(idx) => {
                    let layout_computer =
                        FullLayoutComputer::new(self.ty_ctx, self.target_info.clone());
                    let field_offset_bytes =
                        if let Ok(layout) = layout_computer.layout_of(current_ty) {
                            match &layout.fields {
                                FieldsShape::Arbitrary { offsets } => {
                                    offsets.get(FieldIdx::from_raw(idx.to_raw())).map(|s| s.0)
                                }
                                _ => None,
                            }
                        } else {
                            None
                        };
                    let mut ptr = if let Some(offset) = field_offset_bytes {
                        let i8_ptr = self.context.ptr_type(inkwell::AddressSpace::default());
                        let base_i8 = self
                            .builder
                            .build_bit_cast(ptr, i8_ptr, "field_base_i8")
                            .expect("bitcast to i8* failed")
                            .into_pointer_value();
                        let offset_val = self.llvm_int_type(64).const_int(offset, false);
                        unsafe {
                            self.builder
                                .build_in_bounds_gep(
                                    self.context.i8_type(),
                                    base_i8,
                                    &[offset_val],
                                    "field_offset",
                                )
                                .expect("field offset GEP failed")
                        }
                    } else {
                        let field_idx = idx.to_raw() as u64;
                        let i32_type = self.llvm_int_type(32);
                        let zero = i32_type.const_zero();
                        let field_index = i32_type.const_int(field_idx, false);
                        let llvm_ty = self.llvm_type_for_ty(current_ty);
                        unsafe {
                            self.builder
                                .build_in_bounds_gep(
                                    llvm_ty,
                                    ptr,
                                    &[zero, field_index],
                                    "field_gep",
                                )
                                .expect("field gep failed")
                        }
                    };
                    let field_ty = match self.ty_ctx.ty_kind(current_ty) {
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
                        TyKind::Adt(adt_id, _) => {
                            if let Some(adt_def) = self.ty_ctx.adt_def(*adt_id) {
                                if let Some(variant) = adt_def.variants.first() {
                                    variant
                                        .fields
                                        .iter()
                                        .nth(idx.to_raw() as usize)
                                        .map(|f| f.ty)
                                        .unwrap_or(Ty::ERROR)
                                } else {
                                    Ty::ERROR
                                }
                            } else {
                                Ty::ERROR
                            }
                        }
                        _ => Ty::ERROR,
                    };
                    if field_ty != Ty::ERROR {
                        let _field_llvm_ty = self.llvm_type_for_ty(field_ty);
                        let field_ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                        ptr = self
                            .builder
                            .build_bit_cast(ptr, field_ptr_ty, "field_ptr")
                            .expect("bitcast to field type failed")
                            .into_pointer_value();
                        current_ty = field_ty;
                    }
                }
                ProjectionElem::Index(local_idx) => {
                    let index_ptr = self.get_local_ptr(*local_idx);
                    let i64_ty = self.llvm_int_type(64);
                    let index_val = self
                        .builder
                        .build_load(i64_ty, index_ptr, "index_load")
                        .expect("index load failed")
                        .into_int_value();
                    let elem_ty = match self.ty_ctx.ty_kind(current_ty) {
                        TyKind::Array(elem, _) => *elem,
                        TyKind::Slice(elem) => *elem,
                        _ => {
                            return Err(vec![GlyimDiagnostic::internal_error(format!(
                                "internal compiler error: Index projection on non-array/slice type {:?}",
                                current_ty
                            ))]);
                        }
                    };
                    if let TyKind::Slice(_) = self.ty_ctx.ty_kind(current_ty) {
                        let i8_ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                        let zero_i64 = i64_ty.const_zero();
                        let field0_ptr = unsafe {
                            self.builder
                                .build_in_bounds_gep(
                                    self.llvm_type_for_ty(current_ty),
                                    ptr,
                                    &[zero_i64, zero_i64],
                                    "slice_field0_gep",
                                )
                                .expect("slice field0 gep failed")
                        };
                        let data_ptr = self
                            .builder
                            .build_load(i8_ptr_ty, field0_ptr, "slice_data_ptr")
                            .expect("load data ptr failed")
                            .into_pointer_value();
                        let elem_llvm_ty = self.llvm_type_for_ty(elem_ty);
                        let elem_ptr = unsafe {
                            self.builder
                                .build_in_bounds_gep(
                                    elem_llvm_ty,
                                    data_ptr,
                                    &[index_val],
                                    "slice_elem_gep",
                                )
                                .expect("slice elem gep failed")
                        };
                        ptr = elem_ptr;
                        current_ty = elem_ty;
                    } else {
                        let llvm_ty = self.llvm_type_for_ty(current_ty);
                        let zero_i64 = i64_ty.const_zero();
                        ptr = unsafe {
                            self.builder
                                .build_in_bounds_gep(
                                    llvm_ty,
                                    ptr,
                                    &[zero_i64, index_val],
                                    "array_index_gep",
                                )
                                .expect("array index gep failed")
                        };
                        current_ty = elem_ty;
                    }
                }
                ProjectionElem::Downcast(variant_idx) => {
                    let layout_computer =
                        FullLayoutComputer::new(self.ty_ctx, self.target_info.clone());
                    let layout = layout_computer
                        .layout_of(current_ty)
                        .expect("layout_of failed for downcast");
                    if let VariantsShape::Multiple {
                        tag_size,
                        tag_align,
                        tag_encoding,
                        variants,
                        ..
                    } = &layout.variants
                    {
                        let vi = variant_idx.to_raw() as usize;
                        if vi >= variants.len() {
                            panic!("Variant index {} out of bounds for {:?}", vi, current_ty);
                        }
                        // For `Direct` tag encoding the variant's data starts
                        // *after* the tag (aligned to the variant's own
                        // alignment). For `Niche` encoding there is no
                        // separate tag prefix at all -- every variant's
                        // fields (including the "untagged" storage variant)
                        // live at the same offsets, see `build_niche_layout`
                        // in glyim-layout, which clones the untagged
                        // variant's own field offsets directly onto the
                        // outer enum layout.
                        let data_start = match tag_encoding {
                            TagEncoding::Direct => {
                                Size(tag_size.0)
                                    .align_to(*tag_align)
                                    .align_to(variants[vi].align)
                                    .0
                            }
                            TagEncoding::Niche { .. } => 0,
                        };
                        let i8_ptr = self.context.ptr_type(inkwell::AddressSpace::default());
                        let base_i8 = self
                            .builder
                            .build_bit_cast(ptr, i8_ptr, "downcast_base_i8")
                            .expect("bitcast failed")
                            .into_pointer_value();
                        let offset_val = self.llvm_int_type(64).const_int(data_start, false);
                        ptr = unsafe {
                            self.builder
                                .build_in_bounds_gep(
                                    self.context.i8_type(),
                                    base_i8,
                                    &[offset_val],
                                    "variant_data_ptr",
                                )
                                .expect("downcast GEP failed")
                        };
                    }
                }
                ProjectionElem::ConstantIndex {
                    offset,
                    min_length: _,
                    from_end,
                } => {
                    let index_val = if *from_end {
                        let len = match self.ty_ctx.ty_kind(current_ty) {
                            TyKind::Array(_, const_val) => {
                                let n = match &const_val.kind {
                                    ConstKind::Uint(n) => *n as u64,
                                    ConstKind::Int(n) => *n as u64,
                                    _ => 0,
                                };
                                self.llvm_int_type(64).const_int(n, false)
                            }
                            TyKind::Slice(_) => {
                                let llvm_ty = self.llvm_type_for_ty(current_ty);
                                let val = self
                                    .builder
                                    .build_load(llvm_ty, ptr, "slice_len_load")
                                    .expect("load failed");
                                let struct_val = match val {
                                    BasicValueEnum::StructValue(s) => s,
                                    _ => panic!("slice value not a struct"),
                                };

                                self.builder
                                    .build_extract_value(struct_val, 1, "slice_len_extract")
                                    .expect("extract failed")
                                    .into_int_value()
                            }
                            _ => {
                                return Err(vec![GlyimDiagnostic::internal_error(format!(
                                    "internal compiler error: ConstantIndex on non-array/slice type {:?}",
                                    current_ty
                                ))]);
                            }
                        };
                        let offset_val = self.llvm_int_type(64).const_int(*offset, false);
                        self.builder
                            .build_int_sub(len, offset_val, "sub_index")
                            .expect("sub failed")
                    } else {
                        self.llvm_int_type(64).const_int(*offset, false)
                    };
                    let data_ptr = if let TyKind::Slice(_) = self.ty_ctx.ty_kind(current_ty) {
                        let llvm_ty = self.llvm_type_for_ty(current_ty);
                        let val = self
                            .builder
                            .build_load(llvm_ty, ptr, "slice_load")
                            .expect("load failed");
                        let struct_val = match val {
                            BasicValueEnum::StructValue(s) => s,
                            _ => panic!("slice value not a struct"),
                        };

                        self.builder
                            .build_extract_value(struct_val, 0, "slice_data_ptr")
                            .expect("extract failed")
                            .into_pointer_value()
                    } else {
                        ptr
                    };
                    let elem_ty = match self.ty_ctx.ty_kind(current_ty) {
                        TyKind::Array(elem, _) | TyKind::Slice(elem) => *elem,
                        _ => {
                            return Err(vec![GlyimDiagnostic::internal_error(format!(
                                "internal compiler error: ConstantIndex on non-array/slice type {:?}",
                                current_ty
                            ))]);
                        }
                    };
                    let elem_llvm_ty = self.llvm_type_for_ty(elem_ty);
                    ptr = unsafe {
                        self.builder
                            .build_in_bounds_gep(
                                elem_llvm_ty,
                                data_ptr,
                                &[index_val],
                                "const_index_gep",
                            )
                            .expect("const index gep failed")
                    };
                    current_ty = elem_ty;
                }
                ProjectionElem::Subslice { from, to, from_end } => {
                    // The result of a Subslice projection is itself a
                    // *slice value* `{ data_ptr, len }` -- even when the
                    // base being sliced was a fixed-size array. Since
                    // `place_ptr` can only ever hand back a single
                    // `PointerValue` (not a pointer+length pair), we
                    // materialize that `{ ptr, len }` value into a fresh
                    // stack temporary here and return a pointer to *that*,
                    // so every downstream consumer of the resulting place
                    // (`lower_operand`'s load, further projections, etc.)
                    // sees a real, correctly-typed slice value at the
                    // address we return -- exactly as if it had always
                    // lived in memory.
                    //
                    // INVARIANT: `Subslice` must be the terminal element of
                    // a place's projection list. This matches how slice
                    // patterns (`[a, b, .., z]`) produce it during pattern
                    // lowering -- the subslice binding (`..rest`) is always
                    // read out whole, never projected into further within
                    // the same `Place`. `slice_desugar` (see glyim-opt)
                    // removes every `Subslice` from MIR, and
                    // `glyim_opt::validate::validate_no_subslice` (de-stubbing
                    // plan §8.7) asserts none survive past that pass --
                    // wired as a debug-gated panic in `optimize()`. So
                    // reaching this code with a `Subslice` is true by
                    // construction (proven by the validator), not by hope.
                    let (data_ptr, base_len) = match self.ty_ctx.ty_kind(current_ty) {
                        TyKind::Slice(_) => {
                            let llvm_ty = self.llvm_type_for_ty(current_ty);
                            let val = self
                                .builder
                                .build_load(llvm_ty, ptr, "subslice_base_load")
                                .expect("load failed");
                            let struct_val = match val {
                                BasicValueEnum::StructValue(s) => s,
                                _ => panic!("slice value not a struct"),
                            };
                            let dp = self
                                .builder
                                .build_extract_value(struct_val, 0, "subslice_data_ptr")
                                .expect("extract failed")
                                .into_pointer_value();
                            let ln = self
                                .builder
                                .build_extract_value(struct_val, 1, "subslice_len")
                                .expect("extract failed")
                                .into_int_value();
                            (dp, ln)
                        }
                        TyKind::Array(_, const_val) => {
                            let n = match &const_val.kind {
                                ConstKind::Uint(n) => *n as u64,
                                ConstKind::Int(n) => *n as u64,
                                _ => 0,
                            };
                            (ptr, self.llvm_int_type(64).const_int(n, false))
                        }
                        other => {
                            return Err(vec![GlyimDiagnostic::internal_error(format!(
                                "internal compiler error: Subslice projection on non-array/slice type {:?}",
                                other
                            ))]);
                        }
                    };
                    let elem_ty = match self.ty_ctx.ty_kind(current_ty) {
                        TyKind::Array(elem, _) | TyKind::Slice(elem) => *elem,
                        _ => unreachable!(),
                    };
                    let i64_ty = self.llvm_int_type(64);
                    let from_val = i64_ty.const_int(*from, false);
                    let to_val = i64_ty.const_int(*to, false);
                    let new_len = if *from_end {
                        // `to` counts back from the end: subslice is
                        // `[from .. base_len - to]`.
                        let end_val = self
                            .builder
                            .build_int_sub(base_len, to_val, "subslice_end")
                            .expect("sub failed");
                        self.builder
                            .build_int_sub(end_val, from_val, "subslice_len")
                            .expect("sub failed")
                    } else {
                        // `to` is an absolute index: subslice is `[from .. to]`.
                        self.builder
                            .build_int_sub(to_val, from_val, "subslice_len")
                            .expect("sub failed")
                    };
                    let elem_llvm_ty = self.llvm_type_for_ty(elem_ty);
                    let new_data_ptr = unsafe {
                        self.builder
                            .build_in_bounds_gep(
                                elem_llvm_ty,
                                data_ptr,
                                &[from_val],
                                "subslice_data_ptr",
                            )
                            .expect("gep failed")
                    };
                    let ptr_ty = self.context.ptr_type(AddressSpace::default());
                    let slice_struct_ty = self
                        .context
                        .struct_type(&[ptr_ty.into(), i64_ty.into()], false);
                    let tmp = self
                        .builder
                        .build_alloca(slice_struct_ty, "subslice_tmp")
                        .expect("alloca failed");
                    let agg = slice_struct_ty.const_zero();
                    let agg = self
                        .builder
                        .build_insert_value(agg, new_data_ptr, 0, "subslice_ptr_insert")
                        .expect("insert failed");
                    let agg = self
                        .builder
                        .build_insert_value(agg, new_len, 1, "subslice_len_insert")
                        .expect("insert failed");
                    self.builder
                        .build_store(tmp, agg.as_basic_value_enum())
                        .expect("store failed");
                    ptr = tmp;
                }
            }
        }
        Ok(ptr)
    }

    fn place_ty(&self, place: &Place) -> Ty {
        place.ty(self.ty_ctx, &self.body.locals)
    }

    fn lower_statement(&mut self, stmt: &Statement) -> CompResult<()> {
        self.set_debug_location(stmt.source_info.span);
        match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                let expected_ty = self.place_ty(place);
                let val = self.lower_rvalue(rvalue, expected_ty)?;
                let ptr = self.place_ptr(place)?;
                self.builder.build_store(ptr, val).expect("store failed");
                Ok(())
            }
            StatementKind::StorageLive(local) => {
                self.alloc_local(*local);
                Ok(())
            }
            StatementKind::StorageDead(_local) => Ok(()),
            StatementKind::Nop => Ok(()),
        }
    }

    /// Lower an rvalue to a value.
    ///
    /// `expected_ty` is the type of the place this rvalue is being assigned
    /// into (`Place::ty`). It is authoritative for `Aggregate` construction:
    /// unlike deriving a struct type ad hoc from the operands' LLVM types,
    /// `expected_ty` lets us go through `glyim-layout` and build the value
    /// with the *exact* field offsets (and, for enums, discriminant/niche
    /// tag) that `llvm_type_for_ty(expected_ty)` and every other place that
    /// reads this type already assume.
    fn lower_rvalue(&self, rvalue: &Rvalue, expected_ty: Ty) -> CompResult<BasicValueEnum<'ctx>> {
        match rvalue {
            Rvalue::Use(operand) => Ok(self.lower_operand(operand)?),
            Rvalue::BinaryOp(op, operands) => {
                let (left, right) = operands.as_ref();
                let l = self.lower_operand(left)?;
                let r = self.lower_operand(right)?;
                let operand_ty = self.operand_ty(left);
                self.lower_binop(*op, l, r, operand_ty)
            }
            Rvalue::UnaryOp(op, operand) => {
                let val = self.lower_operand(operand)?;
                self.lower_unop(*op, operand, val)
            }
            Rvalue::Ref(place, _borrow_kind) => {
                let ptr = self.place_ptr(place)?;
                Ok(ptr.as_basic_value_enum())
            }
            Rvalue::Aggregate(kind, operands) => {
                let mut vals = Vec::new();
                for op in operands {
                    vals.push(self.lower_operand(op)?);
                }
                match kind {
                    AggregateKind::Array(elem_ty) => {
                        let llvm_elem_ty = self.llvm_type_for_ty(*elem_ty);
                        let array_ty = llvm_elem_ty.array_type(vals.len() as u32);
                        let mut agg: inkwell::values::AggregateValueEnum<'ctx> =
                            array_ty.const_zero().into();
                        for (i, val) in vals.into_iter().enumerate() {
                            let val = if val.get_type() == llvm_elem_ty {
                                val
                            } else {
                                self.builder
                                    .build_bit_cast(val, llvm_elem_ty, "array_elem_cast")
                                    .expect("bitcast failed")
                            };
                            agg = self
                                .builder
                                .build_insert_value(agg, val, i as u32, "array_insert")
                                .expect("insert value failed");
                        }
                        Ok(agg.as_basic_value_enum())
                    }
                    AggregateKind::Tuple => self.build_layout_aggregate(expected_ty, None, &vals),
                    AggregateKind::Adt(_adt_id, variant, _substs) => {
                        self.build_layout_aggregate(expected_ty, Some(*variant), &vals)
                    }
                    AggregateKind::Closure(_closure_id, _substs) => {
                        self.build_layout_aggregate(expected_ty, None, &vals)
                    }
                }
            }
            Rvalue::Discriminant(place) => self.lower_discriminant(place),
            Rvalue::Len(place) => {
                let ptr = self.place_ptr(place)?;
                let ty = self.place_ty(place);
                let len = match self.ty_ctx.ty_kind(ty) {
                    TyKind::Array(_, const_val) => {
                        let n = match &const_val.kind {
                            ConstKind::Uint(n) => *n as u64,
                            ConstKind::Int(n) => *n as u64,
                            _ => 0,
                        };
                        self.llvm_int_type(64).const_int(n, false)
                    }
                    TyKind::Slice(_) => {
                        let llvm_ty = self.llvm_type_for_ty(ty);
                        let val = self
                            .builder
                            .build_load(llvm_ty, ptr, "slice_len_load")
                            .expect("load failed");
                        let struct_val = match val {
                            BasicValueEnum::StructValue(s) => s,
                            _ => {
                                return Err(vec![GlyimDiagnostic::internal_error(
                                    "slice value not a struct",
                                )]);
                            }
                        };
                        let len_val = self
                            .builder
                            .build_extract_value(struct_val, 1, "slice_len_extract")
                            .expect("extract value failed");
                        len_val.into_int_value()
                    }
                    _ => panic!("Len on non-array/slice type"),
                };
                Ok(len.as_basic_value_enum())
            }
            Rvalue::Cast(kind, operand, target_ty) => {
                let val = self.lower_operand(operand)?;
                self.lower_cast(*kind, val, *target_ty)
            }
            Rvalue::Repeat(operand, count_const) => {
                let val = self.lower_operand(operand)?;
                let count = match &count_const.kind {
                    MirConstKind::Uint(n) => *n as u32,
                    MirConstKind::Int(n) => *n as u32,
                    _ => 0,
                };
                let elem_ty = val.get_type();
                let array_ty = elem_ty.array_type(count);
                let mut agg: inkwell::values::AggregateValueEnum<'ctx> =
                    array_ty.const_zero().into();
                for i in 0..count {
                    agg = self
                        .builder
                        .build_insert_value(agg, val, i, "repeat_insert")
                        .expect("insert value failed");
                }
                Ok(agg.as_basic_value_enum())
            }
        }
    }

    /// Construct an ADT (struct/enum variant) or closure aggregate value
    /// using real layout information from `glyim-layout`, rather than an
    /// ad hoc struct built purely from the operands' own LLVM types.
    ///
    /// This matters for two reasons:
    /// 1. The ad hoc struct type built from `vals.iter().map(|v|
    ///    v.get_type())` is a completely different (and differently laid
    ///    out) LLVM type than `llvm_type_for_ty(expected_ty)`, which is
    ///    what the destination place's memory was allocated/typed as.
    ///    Storing one shape into memory typed for the other is undefined
    ///    behavior.
    /// 2. For enums, it silently dropped the discriminant/niche tag
    ///    entirely -- constructing *any* multi-variant enum value produced
    ///    a value with no tag and the wrong size, guaranteed to be
    ///    misread by anything that later matches on it (`SwitchInt` over
    ///    `Rvalue::Discriminant`) or downcasts it.
    ///
    /// We build the value by allocating a temporary of the *canonical*
    /// LLVM type, storing each field (and the tag, if this is an enum) at
    /// its correct byte offset via `i8`-GEP, then loading the whole thing
    /// back out as the SSA value `lower_rvalue` is expected to return.
    fn build_layout_aggregate(
        &self,
        agg_ty: Ty,
        variant: Option<VariantIdx>,
        vals: &[BasicValueEnum<'ctx>],
    ) -> CompResult<BasicValueEnum<'ctx>> {
        let layout_computer = FullLayoutComputer::new(self.ty_ctx, self.target_info.clone());
        let layout = layout_computer.layout_of(agg_ty).map_err(|e| {
            vec![GlyimDiagnostic::internal_error(format!(
                "layout error building aggregate: {:?}",
                e
            ))]
        })?;
        let llvm_ty = self.llvm_type_for_ty(agg_ty);
        let tmp = self.builder.build_alloca(llvm_ty, "agg_tmp").map_err(|e| {
            vec![GlyimDiagnostic::internal_error(format!(
                "alloca failed: {:?}",
                e
            ))]
        })?;
        let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let base_i8 = self
            .builder
            .build_bit_cast(tmp, i8_ptr_ty, "agg_base_i8")
            .map_err(|e| {
                vec![GlyimDiagnostic::internal_error(format!(
                    "bitcast failed: {:?}",
                    e
                ))]
            })?
            .into_pointer_value();

        // Resolve which field-offset table to use for storing operands, and
        // (for enums) write the discriminant/niche tag.
        let (field_shape, base_offset): (FieldsShape, u64) = match &layout.variants {
            VariantsShape::Single { .. } => (layout.fields.clone(), 0),
            VariantsShape::Multiple {
                tag_field,
                tag_encoding,
                variants,
                tag_size,
                tag_align,
                ..
            } => {
                let vidx = variant.map(|v| v.to_raw() as usize).unwrap_or(0);
                let variant_layout = variants.get(vidx).ok_or_else(|| {
                    vec![GlyimDiagnostic::internal_error(
                        "variant index out of bounds building aggregate",
                    )]
                })?;
                match tag_encoding {
                    TagEncoding::Direct => {
                        // Tag lives at offset 0; variant data starts right
                        // after it, aligned to the variant's own alignment
                        // (mirrors `direct_tag_encoding` in glyim-layout).
                        let tag_bits = (tag_size.0 * 8).max(8) as u32;
                        let tag_llvm_ty = self.llvm_int_type(tag_bits);
                        let tag_ptr_raw = unsafe {
                            self.builder.build_in_bounds_gep(
                                self.context.i8_type(),
                                base_i8,
                                &[self.llvm_int_type(64).const_int(0, false)],
                                "tag_ptr_raw",
                            )
                        }
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "GEP failed: {:?}",
                                e
                            ))]
                        })?;
                        let discr_value = tag_llvm_ty.const_int(vidx as u64, false);
                        self.builder
                            .build_store(tag_ptr_raw, discr_value)
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "store tag failed: {:?}",
                                    e
                                ))]
                            })?;
                        let data_start = Size(tag_size.0)
                            .align_to(*tag_align)
                            .align_to(variant_layout.align)
                            .0;
                        (variant_layout.fields.clone(), data_start)
                    }
                    TagEncoding::Niche {
                        untagged_variant,
                        niche_variants,
                        niche_start,
                    } => {
                        if vidx != *untagged_variant as usize {
                            // Write `niche_start + (vidx - niche_variants.start())`
                            // into the tag field's slot. That slot's offset
                            // comes from the *outer* enum layout's own
                            // `fields`, which `build_niche_layout` (in
                            // glyim-layout) sets to the untagged variant's
                            // field offsets directly -- there is no separate
                            // tag prefix for niche encoding.
                            let rel = vidx as u128 - *niche_variants.start() as u128;
                            let niche_value = niche_start.wrapping_add(rel);
                            let tag_offset = match &layout.fields {
                                FieldsShape::Arbitrary { offsets } => offsets
                                    .get(FieldIdx::from_raw(*tag_field))
                                    .map(|s| s.0)
                                    .unwrap_or(0),
                                _ => 0,
                            };
                            let tag_bits = (tag_size.0 * 8).max(8) as u32;
                            let tag_llvm_ty = self.llvm_int_type(tag_bits);
                            let tag_ptr_raw = unsafe {
                                self.builder.build_in_bounds_gep(
                                    self.context.i8_type(),
                                    base_i8,
                                    &[self.llvm_int_type(64).const_int(tag_offset, false)],
                                    "niche_ptr_raw",
                                )
                            }
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "GEP failed: {:?}",
                                    e
                                ))]
                            })?;
                            // `niche_value`/`niche_start` are u128 so that
                            // pointer- or char-sized niches can be encoded
                            // without loss; truncate to the tag's actual
                            // width when storing.
                            let niche_lo = (niche_value & u128::from(u64::MAX)) as u64;
                            let discr_value = tag_llvm_ty.const_int(niche_lo, false);
                            self.builder
                                .build_store(tag_ptr_raw, discr_value)
                                .map_err(|e| {
                                    vec![GlyimDiagnostic::internal_error(format!(
                                        "store niche failed: {:?}",
                                        e
                                    ))]
                                })?;
                        }
                        // Data fields for every variant (including the
                        // untagged one) live at the same offsets -- no
                        // separate tag prefix.
                        (variant_layout.fields.clone(), 0)
                    }
                }
            }
        };

        let offsets: Vec<u64> = match &field_shape {
            FieldsShape::Arbitrary { offsets } => offsets.iter().map(|s| s.0).collect(),
            FieldsShape::Primitive => vec![0],
            FieldsShape::Array { stride, count } => (0..*count).map(|i| i * stride.0).collect(),
        };
        for (i, val) in vals.iter().enumerate() {
            let byte_offset = base_offset + offsets.get(i).copied().unwrap_or(0);
            let field_ptr_raw = unsafe {
                self.builder.build_in_bounds_gep(
                    self.context.i8_type(),
                    base_i8,
                    &[self.llvm_int_type(64).const_int(byte_offset, false)],
                    "agg_field_raw",
                )
            }
            .map_err(|e| {
                vec![GlyimDiagnostic::internal_error(format!(
                    "GEP failed: {:?}",
                    e
                ))]
            })?;
            let field_ptr = self
                .builder
                .build_bit_cast(
                    field_ptr_raw,
                    self.context.ptr_type(AddressSpace::default()),
                    "agg_field_ptr",
                )
                .map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "bitcast failed: {:?}",
                        e
                    ))]
                })?
                .into_pointer_value();
            self.builder.build_store(field_ptr, *val).map_err(|e| {
                vec![GlyimDiagnostic::internal_error(format!(
                    "store field failed: {:?}",
                    e
                ))]
            })?;
        }
        self.builder
            .build_load(llvm_ty, tmp, "agg_load")
            .map_err(|e| {
                vec![GlyimDiagnostic::internal_error(format!(
                    "load failed: {:?}",
                    e
                ))]
            })
    }

    /// Read the discriminant of an enum place using its real layout,
    /// handling both `Direct` and niche-optimized tag encodings.
    ///
    /// The previous implementation loaded the *whole* place as a struct and
    /// blindly extracted field 0, which happened to work only for `Direct`
    /// encoding where `llvm_type_for_ty` puts the tag first, and was
    /// simply wrong for niche encoding (there is no explicit tag field to
    /// extract at all -- the discriminant must be *computed* from the
    /// niche field's value).
    fn lower_discriminant(&self, place: &Place) -> CompResult<BasicValueEnum<'ctx>> {
        let ptr = self.place_ptr(place)?;
        let ty = self.place_ty(place);
        let layout_computer = FullLayoutComputer::new(self.ty_ctx, self.target_info.clone());
        let layout = layout_computer.layout_of(ty).map_err(|e| {
            vec![GlyimDiagnostic::internal_error(format!(
                "layout error for discriminant: {:?}",
                e
            ))]
        })?;
        let i64_ty = self.llvm_int_type(64);
        match &layout.variants {
            VariantsShape::Single { index } => {
                Ok(i64_ty.const_int(*index as u64, false).as_basic_value_enum())
            }
            VariantsShape::Multiple {
                tag_field,
                tag_encoding,
                tag_size,
                ..
            } => {
                let tag_offset = match &layout.fields {
                    FieldsShape::Arbitrary { offsets } => offsets
                        .get(FieldIdx::from_raw(*tag_field))
                        .map(|s| s.0)
                        .unwrap_or(0),
                    _ => 0,
                };
                let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
                let base_i8 = self
                    .builder
                    .build_bit_cast(ptr, i8_ptr_ty, "discr_base_i8")
                    .map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "bitcast failed: {:?}",
                            e
                        ))]
                    })?
                    .into_pointer_value();
                let offset_val = i64_ty.const_int(tag_offset, false);
                let tag_ptr = unsafe {
                    self.builder.build_in_bounds_gep(
                        self.context.i8_type(),
                        base_i8,
                        &[offset_val],
                        "discr_tag_ptr",
                    )
                }
                .map_err(|e| {
                    vec![GlyimDiagnostic::internal_error(format!(
                        "GEP failed: {:?}",
                        e
                    ))]
                })?;
                let tag_bits = (tag_size.0 * 8).max(8) as u32;
                let tag_llvm_ty = self.llvm_int_type(tag_bits);
                let tag_val = self
                    .builder
                    .build_load(tag_llvm_ty, tag_ptr, "discr_tag_load")
                    .map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "load failed: {:?}",
                            e
                        ))]
                    })?
                    .into_int_value();
                let tag64 = if tag_bits < 64 {
                    self.builder
                        .build_int_z_extend(tag_val, i64_ty, "discr_zext")
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "zext failed: {:?}",
                                e
                            ))]
                        })?
                } else {
                    tag_val
                };
                match tag_encoding {
                    TagEncoding::Direct => Ok(tag64.as_basic_value_enum()),
                    TagEncoding::Niche {
                        untagged_variant,
                        niche_variants,
                        niche_start,
                    } => {
                        // Map the raw niche-field value back to a variant
                        // ordinal:
                        //   in [niche_start, niche_start + niche_len) -> that variant
                        //   otherwise                                 -> untagged_variant
                        let niche_len = (*niche_variants.end() as u128
                            - *niche_variants.start() as u128
                            + 1) as u64;
                        let start_lo = (*niche_start & u128::from(u64::MAX)) as u64;
                        let start_val = i64_ty.const_int(start_lo, false);
                        let end_val = i64_ty.const_int(start_lo.wrapping_add(niche_len), false);
                        let ge_start = self
                            .builder
                            .build_int_compare(
                                inkwell::IntPredicate::UGE,
                                tag64,
                                start_val,
                                "niche_ge",
                            )
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "cmp failed: {:?}",
                                    e
                                ))]
                            })?;
                        let lt_end = self
                            .builder
                            .build_int_compare(
                                inkwell::IntPredicate::ULT,
                                tag64,
                                end_val,
                                "niche_lt",
                            )
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "cmp failed: {:?}",
                                    e
                                ))]
                            })?;
                        let in_range = self
                            .builder
                            .build_and(ge_start, lt_end, "niche_in_range")
                            .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "and failed: {:?}",
                                e
                            ))]
                        })?;
                        let rel = self
                            .builder
                            .build_int_sub(tag64, start_val, "niche_rel")
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "sub failed: {:?}",
                                    e
                                ))]
                            })?;
                        let variant_from_niche = self
                            .builder
                            .build_int_add(
                                rel,
                                i64_ty.const_int(*niche_variants.start() as u64, false),
                                "niche_variant",
                            )
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "add failed: {:?}",
                                    e
                                ))]
                            })?;
                        let untagged_val = i64_ty.const_int(*untagged_variant as u64, false);
                        let result = self
                            .builder
                            .build_select(
                                in_range,
                                variant_from_niche,
                                untagged_val,
                                "discr_select",
                            )
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "select failed: {:?}",
                                    e
                                ))]
                            })?;
                        Ok(result)
                    }
                }
            }
        }
    }

    fn is_signed_int_ty(&self, ty: Ty) -> bool {
        matches!(self.ty_ctx.ty_kind(ty), TyKind::Int(_))
    }

    fn lower_binop(
        &self,
        op: BinOp,
        l: BasicValueEnum<'ctx>,
        r: BasicValueEnum<'ctx>,
        operand_ty: Ty,
    ) -> CompResult<BasicValueEnum<'ctx>> {
        use inkwell::FloatPredicate;
        use inkwell::IntPredicate;

        match op {
            BinOp::Add => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_int_add(l.into_int_value(), r.into_int_value(), "add")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "add failed: {:?}",
                                e
                            ))]
                        })
                } else if l.is_float_value() && r.is_float_value() {
                    self.builder
                        .build_float_add(l.into_float_value(), r.into_float_value(), "add")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "add failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "add: mismatched types",
                    )])
                }
            }
            BinOp::Sub => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_int_sub(l.into_int_value(), r.into_int_value(), "sub")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "sub failed: {:?}",
                                e
                            ))]
                        })
                } else if l.is_float_value() && r.is_float_value() {
                    self.builder
                        .build_float_sub(l.into_float_value(), r.into_float_value(), "sub")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "sub failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "sub: mismatched types",
                    )])
                }
            }
            BinOp::Mul => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_int_mul(l.into_int_value(), r.into_int_value(), "mul")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "mul failed: {:?}",
                                e
                            ))]
                        })
                } else if l.is_float_value() && r.is_float_value() {
                    self.builder
                        .build_float_mul(l.into_float_value(), r.into_float_value(), "mul")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "mul failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "mul: mismatched types",
                    )])
                }
            }
            BinOp::Div => {
                if l.is_int_value() && r.is_int_value() {
                    let result = if self.is_signed_int_ty(operand_ty) {
                        self.builder.build_int_signed_div(
                            l.into_int_value(),
                            r.into_int_value(),
                            "div",
                        )
                    } else {
                        self.builder.build_int_unsigned_div(
                            l.into_int_value(),
                            r.into_int_value(),
                            "div",
                        )
                    };
                    result.map(|v| v.as_basic_value_enum()).map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "div failed: {:?}",
                            e
                        ))]
                    })
                } else if l.is_float_value() && r.is_float_value() {
                    self.builder
                        .build_float_div(l.into_float_value(), r.into_float_value(), "div")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "div failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "div: mismatched types",
                    )])
                }
            }
            BinOp::Rem => {
                if l.is_int_value() && r.is_int_value() {
                    let result = if self.is_signed_int_ty(operand_ty) {
                        self.builder.build_int_signed_rem(
                            l.into_int_value(),
                            r.into_int_value(),
                            "rem",
                        )
                    } else {
                        self.builder.build_int_unsigned_rem(
                            l.into_int_value(),
                            r.into_int_value(),
                            "rem",
                        )
                    };
                    result.map(|v| v.as_basic_value_enum()).map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "rem failed: {:?}",
                            e
                        ))]
                    })
                } else if l.is_float_value() && r.is_float_value() {
                    self.builder
                        .build_float_rem(l.into_float_value(), r.into_float_value(), "rem")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "rem failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "rem: mismatched types",
                    )])
                }
            }
            BinOp::Eq => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_int_compare(
                            IntPredicate::EQ,
                            l.into_int_value(),
                            r.into_int_value(),
                            "eq",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "eq failed: {:?}",
                                e
                            ))]
                        })
                } else if l.is_float_value() && r.is_float_value() {
                    self.builder
                        .build_float_compare(
                            FloatPredicate::OEQ,
                            l.into_float_value(),
                            r.into_float_value(),
                            "eq",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "eq failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "eq: mismatched types",
                    )])
                }
            }
            BinOp::Ne => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_int_compare(
                            IntPredicate::NE,
                            l.into_int_value(),
                            r.into_int_value(),
                            "ne",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "ne failed: {:?}",
                                e
                            ))]
                        })
                } else if l.is_float_value() && r.is_float_value() {
                    self.builder
                        .build_float_compare(
                            FloatPredicate::ONE,
                            l.into_float_value(),
                            r.into_float_value(),
                            "ne",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "ne failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "ne: mismatched types",
                    )])
                }
            }
            BinOp::Lt => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_int_compare(
                            IntPredicate::SLT,
                            l.into_int_value(),
                            r.into_int_value(),
                            "lt",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "lt failed: {:?}",
                                e
                            ))]
                        })
                } else if l.is_float_value() && r.is_float_value() {
                    self.builder
                        .build_float_compare(
                            FloatPredicate::OLT,
                            l.into_float_value(),
                            r.into_float_value(),
                            "lt",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "lt failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "lt: mismatched types",
                    )])
                }
            }
            BinOp::Gt => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_int_compare(
                            IntPredicate::SGT,
                            l.into_int_value(),
                            r.into_int_value(),
                            "gt",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "gt failed: {:?}",
                                e
                            ))]
                        })
                } else if l.is_float_value() && r.is_float_value() {
                    self.builder
                        .build_float_compare(
                            FloatPredicate::OGT,
                            l.into_float_value(),
                            r.into_float_value(),
                            "gt",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "gt failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "gt: mismatched types",
                    )])
                }
            }
            BinOp::LtEq => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_int_compare(
                            IntPredicate::SLE,
                            l.into_int_value(),
                            r.into_int_value(),
                            "le",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "le failed: {:?}",
                                e
                            ))]
                        })
                } else if l.is_float_value() && r.is_float_value() {
                    self.builder
                        .build_float_compare(
                            FloatPredicate::OLE,
                            l.into_float_value(),
                            r.into_float_value(),
                            "le",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "le failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "le: mismatched types",
                    )])
                }
            }
            BinOp::GtEq => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_int_compare(
                            IntPredicate::SGE,
                            l.into_int_value(),
                            r.into_int_value(),
                            "ge",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "ge failed: {:?}",
                                e
                            ))]
                        })
                } else if l.is_float_value() && r.is_float_value() {
                    self.builder
                        .build_float_compare(
                            FloatPredicate::OGE,
                            l.into_float_value(),
                            r.into_float_value(),
                            "ge",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "ge failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "ge: mismatched types",
                    )])
                }
            }
            BinOp::And => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_and(l.into_int_value(), r.into_int_value(), "and")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "and failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "and: expected integer types",
                    )])
                }
            }
            BinOp::Or => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_or(l.into_int_value(), r.into_int_value(), "or")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "or failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "or: expected integer types",
                    )])
                }
            }
            BinOp::BitAnd => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_and(l.into_int_value(), r.into_int_value(), "bitand")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "bitand failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "bitand: expected integer types",
                    )])
                }
            }
            BinOp::BitOr => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_or(l.into_int_value(), r.into_int_value(), "bitor")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "bitor failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "bitor: expected integer types",
                    )])
                }
            }
            BinOp::BitXor => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_xor(l.into_int_value(), r.into_int_value(), "bitxor")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "bitxor failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "bitxor: expected integer types",
                    )])
                }
            }
            BinOp::Shl => {
                if l.is_int_value() && r.is_int_value() {
                    self.builder
                        .build_left_shift(l.into_int_value(), r.into_int_value(), "shl")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "shl failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "shl: expected integer types",
                    )])
                }
            }
            BinOp::Shr => {
                if l.is_int_value() && r.is_int_value() {
                    let signed = self.is_signed_int_ty(operand_ty);
                    self.builder
                        .build_right_shift(l.into_int_value(), r.into_int_value(), signed, "shr")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "shr failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "shr: expected integer types",
                    )])
                }
            }
        }
    }
    fn lower_unop(
        &self,
        op: UnOp,
        operand: &Operand,
        val: BasicValueEnum<'ctx>,
    ) -> CompResult<BasicValueEnum<'ctx>> {
        match op {
            UnOp::Not => {
                if val.is_int_value() {
                    self.builder
                        .build_not(val.into_int_value(), "not")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "not failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "not: expected integer type",
                    )])
                }
            }
            UnOp::Neg => {
                if val.is_int_value() {
                    self.builder
                        .build_int_neg(val.into_int_value(), "neg")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "neg failed: {:?}",
                                e
                            ))]
                        })
                } else if val.is_float_value() {
                    self.builder
                        .build_float_neg(val.into_float_value(), "neg")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "neg failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "neg: expected numeric type",
                    )])
                }
            }
            UnOp::Deref => {
                let ty = self.operand_ty(operand);
                let inner_ty = match self.ty_ctx.ty_kind(ty) {
                    TyKind::Ref(_, inner, _) | TyKind::RawPtr(inner, _) => *inner,
                    _ => Ty::ERROR,
                };
                if val.is_pointer_value() && inner_ty != Ty::ERROR {
                    let llvm_ty = self.llvm_type_for_ty(inner_ty);
                    self.builder
                        .build_load(llvm_ty, val.into_pointer_value(), "deref_load")
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "deref load failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Ok(val)
                }
            }
        }
    }

    fn lower_cast(
        &self,
        kind: CastKind,
        val: BasicValueEnum<'ctx>,
        target_ty: Ty,
    ) -> CompResult<BasicValueEnum<'ctx>> {
        let target_llvm_ty = self.llvm_type_for_ty(target_ty);
        match kind {
            CastKind::IntToInt => {
                if val.get_type() == target_llvm_ty {
                    Ok(val)
                } else if val.is_int_value() && target_llvm_ty.is_int_type() {
                    let int_val = val.into_int_value();
                    let target_bits = target_llvm_ty.into_int_type().get_bit_width();
                    let src_bits = int_val.get_type().get_bit_width();
                    if target_bits == src_bits {
                        Ok(int_val.as_basic_value_enum())
                    } else if target_bits > src_bits {
                        self.builder
                            .build_int_z_extend(int_val, target_llvm_ty.into_int_type(), "zext")
                            .map(|v| v.as_basic_value_enum())
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "zext failed: {:?}",
                                    e
                                ))]
                            })
                    } else {
                        self.builder
                            .build_int_truncate(int_val, target_llvm_ty.into_int_type(), "trunc")
                            .map(|v| v.as_basic_value_enum())
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "trunc failed: {:?}",
                                    e
                                ))]
                            })
                    }
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "IntToInt cast on non-integer",
                    )])
                }
            }
            CastKind::FloatToInt => {
                if val.is_float_value() && target_llvm_ty.is_int_type() {
                    let float_val = val.into_float_value();
                    self.builder
                        .build_float_to_signed_int(
                            float_val,
                            target_llvm_ty.into_int_type(),
                            "fptosi",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "fptosi failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "FloatToInt cast on non-float",
                    )])
                }
            }
            CastKind::IntToFloat => {
                if val.is_int_value() && target_llvm_ty.is_float_type() {
                    let int_val = val.into_int_value();
                    self.builder
                        .build_signed_int_to_float(
                            int_val,
                            target_llvm_ty.into_float_type(),
                            "sitofp",
                        )
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "sitofp failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "IntToFloat cast on non-int",
                    )])
                }
            }
            CastKind::FloatToFloat => {
                if val.is_float_value() && target_llvm_ty.is_float_type() {
                    let float_val = val.into_float_value();
                    let target_float_ty = target_llvm_ty.into_float_type();
                    let src_is_f64 = float_val.get_type() == self.context.f64_type();
                    let target_is_f64 = target_float_ty == self.context.f64_type();
                    if src_is_f64 == target_is_f64 {
                        Ok(float_val.as_basic_value_enum())
                    } else if !src_is_f64 && target_is_f64 {
                        self.builder
                            .build_float_ext(float_val, target_float_ty, "fpext")
                            .map(|v| v.as_basic_value_enum())
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "fpext failed: {:?}",
                                    e
                                ))]
                            })
                    } else {
                        self.builder
                            .build_float_trunc(float_val, target_float_ty, "fptrunc")
                            .map(|v| v.as_basic_value_enum())
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "fptrunc failed: {:?}",
                                    e
                                ))]
                            })
                    }
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "FloatToFloat cast on non-float",
                    )])
                }
            }
            CastKind::PtrToPtr | CastKind::FnPtrToPtr => {
                if val.is_pointer_value() && target_llvm_ty.is_pointer_type() {
                    self.builder
                        .build_bit_cast(val, target_llvm_ty, "bitcast")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "bitcast failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "PtrToPtr cast on non-pointer",
                    )])
                }
            }
            CastKind::PtrToInt => {
                if val.is_pointer_value() && target_llvm_ty.is_int_type() {
                    let ptr_val = val.into_pointer_value();
                    self.builder
                        .build_ptr_to_int(ptr_val, target_llvm_ty.into_int_type(), "ptrtoint")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "ptrtoint failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "PtrToInt cast on non-pointer",
                    )])
                }
            }
            CastKind::IntToPtr => {
                if val.is_int_value() && target_llvm_ty.is_pointer_type() {
                    let int_val = val.into_int_value();
                    self.builder
                        .build_int_to_ptr(int_val, target_llvm_ty.into_pointer_type(), "inttoptr")
                        .map(|v| v.as_basic_value_enum())
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "inttoptr failed: {:?}",
                                e
                            ))]
                        })
                } else {
                    Err(vec![GlyimDiagnostic::internal_error(
                        "IntToPtr cast on non-int",
                    )])
                }
            }
        }
    }

    fn lower_terminator(&mut self, terminator: &Terminator) -> CompResult<()> {
        self.set_debug_location(terminator.source_info.span);
        match &terminator.kind {
            TerminatorKind::Goto { target } => {
                let target_bb = self.bb_map.get(target).unwrap();
                self.builder
                    .build_unconditional_branch(*target_bb)
                    .expect("branch failed");
                Ok(())
            }
            TerminatorKind::SwitchInt {
                discr,
                switch_ty,
                targets,
            } => {
                let discr_val = self.lower_operand(discr)?;
                let discr_int = discr_val.into_int_value();
                let otherwise = *self.bb_map.get(&targets.otherwise()).unwrap();
                let case_ty = self.llvm_type_for_ty(*switch_ty).into_int_type();

                if targets.iter().count() == 0 {
                    self.builder
                        .build_unconditional_branch(otherwise)
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "branch failed: {:?}",
                                e
                            ))]
                        })?;
                } else {
                    let mut branches = Vec::new();
                    for (val, bb) in targets.iter() {
                        let const_val = case_ty.const_int(val as u64, false);
                        let bb_val = *self.bb_map.get(&bb).unwrap();
                        branches.push((const_val, bb_val));
                    }
                    self.builder
                        .build_switch(discr_int, otherwise, &branches)
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "switch failed: {:?}",
                                e
                            ))]
                        })?;
                }
                Ok(())
            }
            TerminatorKind::Return => {
                let ret_ty = self.body.return_ty;
                if matches!(self.ty_ctx.ty_kind(ret_ty), TyKind::Never | TyKind::Unit)
                    || ret_ty == Ty::NEVER
                    || ret_ty == Ty::UNIT
                {
                    self.builder.build_return(None).expect("return failed");
                } else {
                    let ret_op =
                        glyim_mir::Operand::Move(glyim_mir::Place::new(LocalIdx::from_raw(0)));
                    let val = self.lower_operand(&ret_op)?;
                    self.builder
                        .build_return(Some(&val))
                        .expect("return failed");
                }
                Ok(())
            }
            TerminatorKind::Unreachable => {
                // If we're at the end of a cleanup block that has an active
                // landingpad, "falling off the end" means "nothing local
                // wanted to catch/handle this unwind" -- the correct action
                // is to resume unwinding (`resume`), not to assert
                // unreachable (which would be undefined behavior the very
                // first time a destructor actually runs during a panic).
                if let Some(lp) = self.current_landingpad {
                    self.builder.build_resume(lp).map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "resume failed: {:?}",
                            e
                        ))]
                    })?;
                } else {
                    self.builder
                        .build_unreachable()
                        .expect("unreachable failed");
                }
                Ok(())
            }
            TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                cleanup,
            } => self.lower_call(func, args, destination, target, cleanup),
            TerminatorKind::Assert {
                cond,
                expected,
                target,
                cleanup,
                msg,
            } => {
                let cond_int = self.lower_operand(cond)?.into_int_value();
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
                let target_bb = *self.bb_map.get(target).unwrap();
                let fail_bb = self
                    .context
                    .append_basic_block(self.function, "assert_fail");
                self.builder
                    .build_conditional_branch(cmp, target_bb, fail_bb)
                    .expect("assert br failed");
                self.builder.position_at_end(fail_bb);

                // Route assertion failure through the runtime panic entry
                // point rather than a bare `llvm.trap`, so that (a) it goes
                // through the same path as any other panic, and (b) when a
                // cleanup block exists we can `invoke` it, giving unwinding
                // builds a real unwind edge to run destructors on before the
                // process aborts. `glyim_panic` is `-> !`, so the "normal"
                // continuation of the invoke is never actually reached, but
                // LLVM's `invoke` still requires one.
                let module = self.module;
                let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
                let i64_ty = self.llvm_int_type(64);
                let panic_fn = module.get_function("glyim_panic").unwrap_or_else(|| {
                    let fn_ty = self
                        .context
                        .void_type()
                        .fn_type(&[i8_ptr_ty.into(), i64_ty.into()], false);
                    module.add_function("glyim_panic", fn_ty, None)
                });
                let msg_str = format!("assertion failed: {:?}", msg);
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                msg_str.hash(&mut hasher);
                let hash = hasher.finish();
                let global_name = format!("__glyim_assert_{:016x}", hash);
                let msg_global = module.get_global(&global_name).unwrap_or_else(|| {
                    let const_str = self.context.const_string(msg_str.as_bytes(), true);
                    let i8_type = self.context.i8_type();
                    let str_type = i8_type.array_type(msg_str.len() as u32 + 1);
                    let global = module.add_global(
                        str_type,
                        Some(inkwell::AddressSpace::default()),
                        &global_name,
                    );
                    global.set_initializer(&const_str);
                    global.set_constant(true);
                    global.set_linkage(inkwell::module::Linkage::Private);
                    global
                });
                let msg_ptr = self
                    .builder
                    .build_bit_cast(msg_global.as_pointer_value(), i8_ptr_ty, "msg_ptr")
                    .expect("bitcast failed")
                    .into_pointer_value();
                let msg_len = i64_ty.const_int(msg_str.len() as u64, false);
                let panic_args: Vec<BasicMetadataValueEnum<'ctx>> =
                    vec![msg_ptr.into(), msg_len.into()];

                if let Some(cleanup_bb_idx) = cleanup {
                    let cleanup_bb = *self.bb_map.get(cleanup_bb_idx).unwrap();
                    let unreachable_cont = self
                        .context
                        .append_basic_block(self.function, "assert_panic_cont");
                    let panic_args_vals: Vec<BasicValueEnum<'ctx>> =
                        vec![msg_ptr.as_basic_value_enum(), msg_len.as_basic_value_enum()];
                    self.builder
                        .build_invoke(
                            panic_fn,
                            &panic_args_vals,
                            unreachable_cont,
                            cleanup_bb,
                            "panic_call",
                        )
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "invoke failed: {:?}",
                                e
                            ))]
                        })?;
                    self.builder.position_at_end(unreachable_cont);
                    self.builder.build_unreachable().map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "unreachable failed: {:?}",
                            e
                        ))]
                    })?;
                } else {
                    self.builder
                        .build_call(panic_fn, &panic_args, "panic_call")
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "call failed: {:?}",
                                e
                            ))]
                        })?;
                    self.builder.build_unreachable().map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "unreachable failed: {:?}",
                            e
                        ))]
                    })?;
                }
                Ok(())
            }
            TerminatorKind::Drop {
                place,
                target,
                cleanup,
            } => {
                let ty = self.place_ty(place);
                let needs_drop = self.type_needs_drop(ty);
                let is_owning = self.type_is_owning_pointer(ty);

                let target_bb = *self.bb_map.get(target).unwrap();
                let cleanup_bb = cleanup.map(|c| *self.bb_map.get(&c).unwrap());

                if needs_drop || is_owning {
                    let drop_fn = self
                        .module
                        .get_function("glyim_drop_in_place")
                        .unwrap_or_else(|| {
                            let fn_type = self.context.void_type().fn_type(
                                &[self.context.ptr_type(AddressSpace::default()).into()],
                                false,
                            );
                            self.module
                                .add_function("glyim_drop_in_place", fn_type, None)
                        });

                    let drop_arg = if matches!(
                        self.ty_ctx.ty_kind(ty),
                        TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _)
                    ) {
                        let val = self.lower_operand(&Operand::Move(place.clone()))?;
                        val.into_pointer_value().into()
                    } else {
                        self.place_ptr(place)?.into()
                    };
                    let args_vals: Vec<BasicValueEnum<'ctx>> = vec![drop_arg];

                    if let Some(cleanup_bb) = cleanup_bb {
                        let normal_bb = self.context.append_basic_block(self.function, "drop_cont");
                        self.builder
                            .build_invoke(drop_fn, &args_vals, normal_bb, cleanup_bb, "drop_call")
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "invoke failed: {:?}",
                                    e
                                ))]
                            })?;

                        self.builder.position_at_end(normal_bb);
                        if is_owning {
                            let dealloc_fn = self
                                .module
                                .get_function("glyim_dealloc")
                                .unwrap_or_else(|| {
                                    let fn_type = self.context.void_type().fn_type(
                                        &[
                                            self.context.ptr_type(AddressSpace::default()).into(),
                                            self.llvm_int_type(64).into(),
                                            self.llvm_int_type(64).into(),
                                        ],
                                        false,
                                    );
                                    self.module.add_function("glyim_dealloc", fn_type, None)
                                });
                            let val = self.lower_operand(&Operand::Move(place.clone()))?;
                            let ptr = val.into_pointer_value();
                            let pointee_ty = match self.ty_ctx.ty_kind(ty) {
                                TyKind::Ref(_, inner, _) | TyKind::RawPtr(inner, _) => *inner,
                                _ => unreachable!(),
                            };
                            let layout_computer =
                                FullLayoutComputer::new(self.ty_ctx, self.target_info.clone());
                            if let Ok(layout) = layout_computer.layout_of(pointee_ty) {
                                let size_val =
                                    self.llvm_int_type(64).const_int(layout.size.0, false);
                                let align_val =
                                    self.llvm_int_type(64).const_int(layout.align.0, false);
                                let dealloc_args: Vec<BasicMetadataValueEnum<'ctx>> =
                                    vec![ptr.into(), size_val.into(), align_val.into()];
                                self.builder
                                    .build_call(dealloc_fn, &dealloc_args, "dealloc_call")
                                    .map_err(|e| {
                                        vec![GlyimDiagnostic::internal_error(format!(
                                            "dealloc call failed: {:?}",
                                            e
                                        ))]
                                    })?;
                            }
                        }
                        self.builder
                            .build_unconditional_branch(target_bb)
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "branch failed: {:?}",
                                    e
                                ))]
                            })?;
                    } else {
                        let args_meta: Vec<BasicMetadataValueEnum<'ctx>> =
                            args_vals.iter().map(|&v| v.into()).collect();
                        self.builder
                            .build_call(drop_fn, &args_meta, "drop_call")
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "call failed: {:?}",
                                    e
                                ))]
                            })?;

                        if is_owning {
                            let dealloc_fn = self
                                .module
                                .get_function("glyim_dealloc")
                                .unwrap_or_else(|| {
                                    let fn_type = self.context.void_type().fn_type(
                                        &[
                                            self.context.ptr_type(AddressSpace::default()).into(),
                                            self.llvm_int_type(64).into(),
                                            self.llvm_int_type(64).into(),
                                        ],
                                        false,
                                    );
                                    self.module.add_function("glyim_dealloc", fn_type, None)
                                });
                            let val = self.lower_operand(&Operand::Move(place.clone()))?;
                            let ptr = val.into_pointer_value();
                            let pointee_ty = match self.ty_ctx.ty_kind(ty) {
                                TyKind::Ref(_, inner, _) | TyKind::RawPtr(inner, _) => *inner,
                                _ => unreachable!(),
                            };
                            let layout_computer =
                                FullLayoutComputer::new(self.ty_ctx, self.target_info.clone());
                            if let Ok(layout) = layout_computer.layout_of(pointee_ty) {
                                let size_val =
                                    self.llvm_int_type(64).const_int(layout.size.0, false);
                                let align_val =
                                    self.llvm_int_type(64).const_int(layout.align.0, false);
                                let dealloc_args: Vec<BasicMetadataValueEnum<'ctx>> =
                                    vec![ptr.into(), size_val.into(), align_val.into()];
                                self.builder
                                    .build_call(dealloc_fn, &dealloc_args, "dealloc_call")
                                    .map_err(|e| {
                                        vec![GlyimDiagnostic::internal_error(format!(
                                            "dealloc call failed: {:?}",
                                            e
                                        ))]
                                    })?;
                            }
                        }
                        self.builder
                            .build_unconditional_branch(target_bb)
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "branch failed: {:?}",
                                    e
                                ))]
                            })?;
                    }
                } else {
                    self.builder
                        .build_unconditional_branch(target_bb)
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "branch failed: {:?}",
                                e
                            ))]
                        })?;
                }
                Ok(())
            }
        }
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
        let is_sret = matches!(fn_abi.ret.mode, PassMode::Indirect { .. });
        let fn_type = self.llvm_fn_type_from_sig(&fn_sig);
        let direct_fn_val = match func {
            Operand::Constant(c) => {
                if let MirConstKind::Fn(fn_def_id, _) = &c.kind {
                    let fn_name = format!("__glyim_fn_{}", fn_def_id.to_raw());
                    self.module.get_function(&fn_name)
                } else {
                    None
                }
            }
            _ => None,
        };
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
            let arg_val = self.lower_operand(arg_op)?;
            match arg_abi.mode {
                PassMode::Direct => llvm_args.push(arg_val),
                PassMode::Indirect { .. } => {
                    let llvm_ty = self.llvm_type_for_ty(arg_abi.ty);
                    let tmp_ptr = self
                        .builder
                        .build_alloca(llvm_ty, "arg")
                        .expect("alloca arg failed");
                    self.builder
                        .build_store(tmp_ptr, arg_val)
                        .expect("store arg failed");
                    llvm_args.push(tmp_ptr.as_basic_value_enum());
                }
                PassMode::Ignore => unreachable!(),
                PassMode::Cast { .. } => {
                    let layout = layout_computer.layout_of(arg_abi.ty).map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "Layout error in Cast: {:?}",
                            e
                        ))]
                    })?;
                    let size_bits = layout.size.0 * 8;
                    let cast_ty = self.llvm_int_type(size_bits as u32);
                    let cast_ptr = self
                        .builder
                        .build_alloca(cast_ty, "arg_cast")
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "alloca failed: {:?}",
                                e
                            ))]
                        })?;
                    self.builder.build_store(cast_ptr, arg_val).map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "store failed: {:?}",
                            e
                        ))]
                    })?;
                    let casted_val = self
                        .builder
                        .build_load(cast_ty, cast_ptr, "arg_casted")
                        .map_err(|e| {
                            vec![GlyimDiagnostic::internal_error(format!(
                                "load failed: {:?}",
                                e
                            ))]
                        })?;
                    llvm_args.push(casted_val);
                }
                PassMode::HomogeneousAggregate { .. } | PassMode::Split { .. } => {
                    let struct_val = arg_val.into_struct_value();
                    let num_fields = struct_val.get_type().count_fields();
                    for i in 0..num_fields {
                        let field_val = self
                            .builder
                            .build_extract_value(struct_val, i, "agg_field")
                            .map_err(|e| {
                                vec![GlyimDiagnostic::internal_error(format!(
                                    "extract failed: {:?}",
                                    e
                                ))]
                            })?;
                        llvm_args.push(field_val);
                    }
                }
            }
            arg_idx += 1;
        }
        let metadata_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            llvm_args.iter().map(|v| (*v).into()).collect();
        let use_invoke = cleanup.is_some();
        let call_result = if let Some(fn_val) = direct_fn_val {
            if use_invoke {
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
                    .build_invoke(fn_val, &llvm_args, normal_bb, cleanup_bb, "call")
                    .map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "invoke failed: {:?}",
                            e
                        ))]
                    })?
            } else {
                self.builder
                    .build_call(fn_val, &metadata_args, "call")
                    .map_err(|e| {
                        vec![GlyimDiagnostic::internal_error(format!(
                            "call failed: {:?}",
                            e
                        ))]
                    })?
            }
        } else {
            let func_val = self.lower_operand(func)?.into_pointer_value();
            if use_invoke {
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
                    .build_indirect_invoke(
                        fn_type, func_val, &llvm_args, normal_bb, cleanup_bb, "call",
                    )
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
            }
        };
        let mut param_idx = 1;
        if is_sret {
            let sret_attr = self.context.create_enum_attribute(
                inkwell::attributes::Attribute::get_named_enum_kind_id("sret"),
                0,
            );
            call_result.add_attribute(
                inkwell::attributes::AttributeLoc::Param(param_idx),
                sret_attr,
            );
            param_idx += 1;
        }
        for arg_abi in &fn_abi.args {
            if let PassMode::Indirect { .. } = arg_abi.mode {
                let llvm_ty = self.llvm_type_for_ty(arg_abi.ty);
                let any_ty = match llvm_ty {
                    inkwell::types::BasicTypeEnum::ArrayType(t) => {
                        inkwell::types::AnyTypeEnum::ArrayType(t)
                    }
                    inkwell::types::BasicTypeEnum::FloatType(t) => {
                        inkwell::types::AnyTypeEnum::FloatType(t)
                    }
                    inkwell::types::BasicTypeEnum::IntType(t) => {
                        inkwell::types::AnyTypeEnum::IntType(t)
                    }
                    inkwell::types::BasicTypeEnum::PointerType(t) => {
                        inkwell::types::AnyTypeEnum::PointerType(t)
                    }
                    inkwell::types::BasicTypeEnum::StructType(t) => {
                        inkwell::types::AnyTypeEnum::StructType(t)
                    }
                    inkwell::types::BasicTypeEnum::VectorType(t) => {
                        inkwell::types::AnyTypeEnum::VectorType(t)
                    }
                    inkwell::types::BasicTypeEnum::ScalableVectorType(t) => {
                        inkwell::types::AnyTypeEnum::ScalableVectorType(t)
                    }
                };
                let byval_attr = self.context.create_type_attribute(
                    inkwell::attributes::Attribute::get_named_enum_kind_id("byval"),
                    any_ty,
                );
                call_result.add_attribute(
                    inkwell::attributes::AttributeLoc::Param(param_idx),
                    byval_attr,
                );
            }
            if !matches!(arg_abi.mode, PassMode::Ignore) {
                param_idx += 1;
            }
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
            let dest_ptr = self.place_ptr(destination)?;
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
            let dest_ptr = self.place_ptr(destination)?;
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

    fn operand_ty(&self, operand: &Operand) -> Ty {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => self.place_ty(place),
            Operand::Constant(c) => c.ty,
        }
    }

    /// Emit a landingpad instruction for the cleanup block currently being
    /// positioned at, and record its value in `self.current_landingpad`.
    ///
    /// Glyim has no user-visible `catch` construct (panics are always fatal
    /// -- see `glyim_panic` in glyim-runtime), so every landingpad we emit
    /// is a pure *cleanup* landingpad: it doesn't filter by exception type
    /// at all (`clauses = &[]`, `is_cleanup = true`), it just gives us a
    /// legal place for the unwinder to hand control back to us so that any
    /// `Drop` terminators already present in this cleanup block (emitted by
    /// `glyim-opt::drop_elaboration`) get to run before we `resume` the
    /// unwind (see `TerminatorKind::Unreachable` above).
    fn emit_landingpad(&mut self) -> CompResult<()> {
        let Some(personality_fn) = self._personality_fn else {
            self.current_landingpad = None;
            return Ok(());
        };
        let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let landingpad_ty = self
            .context
            .struct_type(&[i8_ptr_ty.into(), i32_ty.into()], false);
        let landingpad = self
            .builder
            .build_landing_pad(landingpad_ty, personality_fn, &[], true, "lpad")
            .map_err(|e| {
                vec![GlyimDiagnostic::internal_error(format!(
                    "landingpad failed: {:?}",
                    e
                ))]
            })?;

        self.current_landingpad = Some(landingpad);
        Ok(())
    }
    fn type_needs_drop(&self, ty: Ty) -> bool {
        match self.ty_ctx.ty_kind(ty) {
            TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => false,
            TyKind::Array(inner, _) => self.type_needs_drop(*inner),
            TyKind::Tuple(substs) => self.ty_ctx.substitution_args(*substs).iter().any(|arg| {
                if let glyim_type::GenericArg::Ty(t) = arg {
                    self.type_needs_drop(*t)
                } else {
                    false
                }
            }),
            TyKind::Adt(adt_id, _) => {
                if let Some(adt_def) = self.ty_ctx.adt_def(*adt_id) {
                    adt_def
                        .variants
                        .iter()
                        .any(|v| v.fields.iter().any(|f| self.type_needs_drop(f.ty)))
                } else {
                    false
                }
            }
            TyKind::Closure(_, substs) => {
                self.ty_ctx.substitution_args(*substs).iter().any(|arg| {
                    if let glyim_type::GenericArg::Ty(t) = arg {
                        self.type_needs_drop(*t)
                    } else {
                        false
                    }
                })
            }
            TyKind::String => true,
            TyKind::Bool
            | TyKind::Int(_)
            | TyKind::Uint(_)
            | TyKind::Float(_)
            | TyKind::Char
            | TyKind::Never
            | TyKind::Unit
            | TyKind::FnDef(_, _)
            | TyKind::FnPtr(_) => false,
            // Unreachable after TASK-P1-6, but conservative fallback is true.
            TyKind::Dynamic(..)
            | TyKind::Opaque(..)
            | TyKind::Projection(_)
            | TyKind::Param(_)
            | TyKind::Bound(_, _)
            | TyKind::Infer(_) => true,
            _ => false,
        }
    }

    fn type_is_owning_pointer(&self, ty: Ty) -> bool {
        match self.ty_ctx.ty_kind(ty) {
            // INTENTIONAL: only Ref(Mut)/RawPtr(Mut) are owning-pointer-like today;
            // extend this match when an owned-box type is added to glyim-type.
            TyKind::Ref(_, inner, glyim_core::primitives::Mutability::Mut)
            | TyKind::RawPtr(inner, glyim_core::primitives::Mutability::Mut) => {
                !self.ty_ctx.is_copy(*inner)
            }
            _ => false,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_body<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    body: &Body,
    target_info: TargetInfo,
    ty_ctx: &TyCtx,
    debug_info: bool,
    source_map: HashMap<FileId, (String, String)>,
    hygiene: Option<HygieneCtx>,
) -> CompResult<()> {
    let fn_name = format!(
        "func_{}_{}",
        body.owner.krate.to_raw(),
        body.owner.local_id.to_raw()
    );
    let ret_llvm_ty = llvm_type_for_ty(ty_ctx, &target_info, context, body.return_ty)?;
    let void_type = context.void_type();
    let mut param_types = Vec::new();
    for i in 1..=body.arg_count {
        let local_idx = LocalIdx::from_raw(i as u32);
        if let Some(local_decl) = body.locals.get(local_idx) {
            let param_ty = llvm_type_for_ty(ty_ctx, &target_info, context, local_decl.ty)?;
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

    // Apply ABI attributes to function definition parameters.
    // A missing FnSig for a function that has reached codegen is an internal
    // compiler error (not a user error): by the time we lower to LLVM IR every
    // called function must already have had its signature resolved during
    // typeck/HIR lowering. Silently substituting an empty FnSig here produced a
    // wrong-arity LLVM function that crashed far from the real cause, so fail
    // loudly instead.
    let layout_computer = FullLayoutComputer::new(ty_ctx, target_info.clone());
    let fn_def_id = glyim_core::def_id::FnDefId::from_raw(body.owner.local_id.to_raw());
    // Prefer the FnSig registered by typeck. When codegen is driven directly
    // from a Body (unit tests, REPL, incremental single-body lowers) no sig is
    // registered, so derive a fallback from the body's return type. The LLVM
    // function type itself is already built from body.locals/return_ty above,
    // so this fallback only needs to supply the FnAbi (correct for scalar
    // args/returns, which is what direct-body lowers use).
    let fn_sig = match ty_ctx.fn_sig(fn_def_id) {
        Some(sig) => sig.clone(),
        None => FnSig {
            inputs: Substitution::empty(),
            output: body.return_ty,
            c_variadic: false,
            unsafety: Safety::Safe,
            abi: Abi::Glyim,
        },
    };
    if let Ok(fn_abi) = layout_computer.fn_abi_of(&fn_sig) {
        let mut param_idx = 0;
        if matches!(fn_abi.ret.mode, glyim_layout::PassMode::Indirect { .. }) {
            let sret_attr = context.create_enum_attribute(
                inkwell::attributes::Attribute::get_named_enum_kind_id("sret"),
                0,
            );
            function.add_attribute(
                inkwell::attributes::AttributeLoc::Param(param_idx),
                sret_attr,
            );
            param_idx += 1;
        }
        for arg_abi in &fn_abi.args {
            if let glyim_layout::PassMode::Indirect { .. } = arg_abi.mode {
                let llvm_ty = llvm_type_for_ty(ty_ctx, &target_info, context, arg_abi.ty)
                    .unwrap_or(context.i8_type().into());
                let any_ty = match llvm_ty {
                    inkwell::types::BasicTypeEnum::ArrayType(t) => {
                        inkwell::types::AnyTypeEnum::ArrayType(t)
                    }
                    inkwell::types::BasicTypeEnum::FloatType(t) => {
                        inkwell::types::AnyTypeEnum::FloatType(t)
                    }
                    inkwell::types::BasicTypeEnum::IntType(t) => {
                        inkwell::types::AnyTypeEnum::IntType(t)
                    }
                    inkwell::types::BasicTypeEnum::PointerType(t) => {
                        inkwell::types::AnyTypeEnum::PointerType(t)
                    }
                    inkwell::types::BasicTypeEnum::StructType(t) => {
                        inkwell::types::AnyTypeEnum::StructType(t)
                    }
                    inkwell::types::BasicTypeEnum::VectorType(t) => {
                        inkwell::types::AnyTypeEnum::VectorType(t)
                    }
                    inkwell::types::BasicTypeEnum::ScalableVectorType(t) => {
                        inkwell::types::AnyTypeEnum::ScalableVectorType(t)
                    }
                };
                let byval_attr = context.create_type_attribute(
                    inkwell::attributes::Attribute::get_named_enum_kind_id("byval"),
                    any_ty,
                );
                function.add_attribute(
                    inkwell::attributes::AttributeLoc::Param(param_idx),
                    byval_attr,
                );
            }
            param_idx += 1;
        }
    }

    let mut debug_ctx = if debug_info {
        Some(DebugInfoCtx::new(
            context, module, source_map, true, hygiene,
        ))
    } else {
        None
    };
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
    let _ptr_type = context.ptr_type(AddressSpace::default());
    let _i64_type = context
        .custom_width_int_type(NonZeroU32::new(64).unwrap())
        .unwrap();
    let has_cleanup = body.basic_blocks.iter().any(|bb| bb.is_cleanup);
    let personality_fn = match select_personality(&target_info, has_cleanup) {
        Personality::None => None,
        Personality::Itanium | Personality::Seh => {
            // `__gcc_personality_v0` is the Itanium-ABI personality routine
            // provided by libgcc on Linux/glibc (and the `__gxx_personality_v0`
            // family on macOS) targets, which is what the build's linker (see
            // glyim-cli/src/linker.rs, which shells out to `cc`) resolves at
            // link time without extra runtime support. This is declared, not
            // defined, here — exactly like a C/C++ program compiled with
            // `-fexceptions` would.
            //
            // On Windows (`Seh`) the personality is `__CxxFrameHandler3`; the
            // SEH funclet-based landingpad lowering that consumes it is tracked
            // under §19.1 and is not yet wired (the landingpad path below is the
            // Itanium one). For now we still emit the correct personality name
            // so the symbol is declared for the linker.
            let personality_name = match target_info.abi {
                TargetAbi::X86_64Windows | TargetAbi::AArch64Windows => "__CxxFrameHandler3",
                _ => "__gcc_personality_v0",
            };
            let personality_fn_type = context.i32_type().fn_type(&[], true);
            let personality_fn = module
                .get_function(personality_name)
                .unwrap_or_else(|| module.add_function(personality_name, personality_fn_type, None));
            function.set_personality_function(personality_fn);
            Some(personality_fn)
        }
    };
    let mut lowering_ctx = LoweringCtx {
        context,
        builder,
        function,
        module,
        body,
        target_info,
        ty_ctx,
        locals,
        bb_map,
        _personality_fn: personality_fn,
        debug_ctx,
        current_landingpad: None,
    };
    for (local_idx, _local_decl) in body.locals.iter_enumerated() {
        lowering_ctx.alloc_local(local_idx);
    }
    for (bb_idx, bb_data) in body.basic_blocks.iter_enumerated() {
        let llvm_bb = lowering_ctx.bb_map.get(&bb_idx).unwrap();
        lowering_ctx.builder.position_at_end(*llvm_bb);
        // Reset per-block: a landingpad value from a previous cleanup block
        // must never leak into a block that isn't itself a fresh cleanup
        // landing pad.
        lowering_ctx.current_landingpad = None;
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
