//! Type layout computation — sizes, alignments, ABI details.

use glyim_core::abi::ALIGN_MAX;
use glyim_core::arena::IndexVec;
use glyim_core::primitives::{Abi, TargetInfo};
use glyim_type::adt_def::{AdtDef, AdtKind};
use glyim_type::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Size(pub u64);

impl Size {
    pub const ZERO: Size = Size(0);
    pub fn bytes(b: u64) -> Self {
        Size(b)
    }
    pub fn bits(&self) -> u64 {
        self.0.saturating_mul(8)
    }
    pub fn align_to(&self, align: Align) -> Self {
        debug_assert!(align.0 > 0, "alignment must be non-zero");
        let mask = align.0 - 1;
        Size((self.0 + mask) & !mask)
    }
    /// Checked multiplication: returns `None` on overflow.
    pub fn checked_mul(self, rhs: u64) -> Option<Size> {
        self.0.checked_mul(rhs).map(Size)
    }
}

impl std::ops::Add for Size {
    type Output = Size;
    fn add(self, rhs: Size) -> Size {
        Size(self.0.saturating_add(rhs.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Align(pub u64);

impl Align {
    pub const ONE: Align = Align(1);
    pub const EIGHT: Align = Align(8);
    pub fn from_bytes(bytes: u64) -> Self {
        debug_assert!(bytes.is_power_of_two(), "alignment must be a power of two, got {bytes}");
        Align(bytes)
    }
    pub fn max(self, other: Self) -> Self {
        Align(self.0.max(other.0))
    }
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub size: Size,
    pub align: Align,
    pub fields: FieldsShape,
    pub variants: VariantsShape,
    pub is_unsized: bool,
}

impl Layout {
    pub fn scalar(size: Size, align: Align) -> Self {
        Self {
            size,
            align,
            fields: FieldsShape::Primitive,
            variants: VariantsShape::Single { index: 0 },
            is_unsized: false,
        }
    }
    pub fn unit() -> Self {
        Self {
            size: Size::ZERO,
            align: Align::ONE,
            fields: FieldsShape::Arbitrary {
                offsets: IndexVec::new(),
            },
            variants: VariantsShape::Single { index: 0 },
            is_unsized: false,
        }
    }
}

#[derive(Clone, Debug)]
pub enum FieldsShape {
    Primitive,
    Array { stride: Size, count: u64 },
    Arbitrary { offsets: IndexVec<FieldIdx, Size> },
}

#[derive(Clone, Debug)]
pub enum VariantsShape {
    Single {
        index: u32,
    },
    Multiple {
        tag: Ty,
        tag_field: u32,
        tag_encoding: TagEncoding,
        variants: Vec<Layout>,
    },
}

#[derive(Clone, Debug)]
pub enum TagEncoding {
    Direct,
    Niche {
        untagged_variant: u32,
        niche_variants: std::ops::RangeInclusive<u32>,
        niche_start: u128,
    },
}

#[derive(Clone, Debug)]
pub struct FnAbi {
    pub args: Vec<ArgAbi>,
    pub ret: ArgAbi,
    pub conv: CallConvention,
    pub c_variadic: bool,
}

#[derive(Clone, Debug)]
pub struct ArgAbi {
    pub ty: Ty,
    pub layout: Layout,
    pub mode: PassMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PassMode {
    Direct,
    Indirect { meta_attrs: bool },
    Ignore,
    Cast { to: Ty, cast_int: bool },
    HomogeneousAggregate { element_ty: Ty, count: u32 },
    Split { pieces: Vec<PassMode> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallConvention {
    Glyim,
    C,
    System,
}

impl From<Abi> for CallConvention {
    fn from(abi: Abi) -> Self {
        match abi {
            Abi::C => CallConvention::C,
            Abi::Glyim => CallConvention::Glyim,
            Abi::System => CallConvention::System,
        }
    }
}

pub trait LayoutComputer {
    fn layout_of(&self, ty: Ty) -> Result<Layout, LayoutError>;
    fn fn_abi_of(&self, sig: &FnSig) -> Result<FnAbi, LayoutError>;
    fn ptr_size(&self) -> Size;
    fn ptr_align(&self) -> Align;
    fn target_info(&self) -> &TargetInfo;
}

#[derive(Clone, Debug)]
pub enum LayoutError {
    UnknownType(Ty),
    SizeOverflow(Ty),
    Unsized(Ty),
    Cycle(Ty),
    AlignmentExceedsRuntime { ty: Ty, align: u64, max: u64 },
}

pub struct SimpleLayoutComputer<'a> {
    ctx: &'a TyCtx,
    target: TargetInfo,
}

impl<'a> SimpleLayoutComputer<'a> {
    pub fn new(ctx: &'a TyCtx, target: TargetInfo) -> Self {
        const _: () = assert!(ALIGN_MAX >= 8, "ALIGN_MAX must be at least 8");
        Self { ctx, target }
    }

    /// Compute layout for a tuple type (including unit).
    fn layout_tuple(&self, substs: Substitution) -> Result<Layout, LayoutError> {
        let args = self.ctx.substitution_args(substs);
        if args.is_empty() {
            return Ok(Layout::unit());
        }

        let mut offsets = IndexVec::new();
        let mut struct_align = Align::ONE;
        let mut current_offset = Size::ZERO;

        for arg in args {
            if let GenericArg::Ty(field_ty) = arg {
                let field_layout = self.layout_of(*field_ty)?;
                struct_align = struct_align.max(field_layout.align);
                current_offset = current_offset.align_to(field_layout.align);
                offsets.push(current_offset);
                current_offset = current_offset + field_layout.size;
            }
            // Skip lifetime/const args — they don't occupy tuple space
        }

        let size = current_offset.align_to(struct_align);

        Ok(Layout {
            size,
            align: struct_align,
            fields: FieldsShape::Arbitrary { offsets },
            variants: VariantsShape::Single { index: 0 },
            is_unsized: false,
        })
    }

    /// Compute layout for an array type.
    fn layout_array(&self, inner: Ty, count: &Const, outer_ty: Ty) -> Result<Layout, LayoutError> {
        let inner_layout = self.layout_of(inner)?;

        let count_val: u64 = match &count.kind {
            ConstKind::Uint(n) => {
                u64::try_from(*n).map_err(|_| LayoutError::SizeOverflow(outer_ty))?
            }
            ConstKind::Int(n) => {
                if *n < 0 {
                    return Err(LayoutError::SizeOverflow(outer_ty));
                }
                u64::try_from(*n).map_err(|_| LayoutError::SizeOverflow(outer_ty))?
            }
            _ => return Err(LayoutError::UnknownType(outer_ty)),
        };

        let stride = inner_layout.size.align_to(inner_layout.align);
        let size = stride
            .checked_mul(count_val)
            .ok_or(LayoutError::SizeOverflow(outer_ty))?;

        Ok(Layout {
            size,
            align: inner_layout.align,
            fields: FieldsShape::Array {
                stride,
                count: count_val,
            },
            variants: VariantsShape::Single { index: 0 },
            is_unsized: false,
        })
    }

    /// Compute layout for a struct ADT.
    fn layout_struct(&self, adt_def: &AdtDef) -> Result<Layout, LayoutError> {
        let mut offsets = IndexVec::new();
        let mut struct_align = Align::ONE;
        let mut current_offset = Size::ZERO;

        for field in adt_def.fields.iter() {
            let field_layout = self.layout_of(field.ty)?;
            struct_align = struct_align.max(field_layout.align);
            current_offset = current_offset.align_to(field_layout.align);
            offsets.push(current_offset);
            current_offset = current_offset + field_layout.size;
        }

        let size = current_offset.align_to(struct_align);

        Ok(Layout {
            size,
            align: struct_align,
            fields: FieldsShape::Arbitrary { offsets },
            variants: VariantsShape::Single { index: 0 },
            is_unsized: false,
        })
    }

    /// Compute layout for a union ADT (all fields at offset 0, size = max).
    fn layout_union(&self, adt_def: &AdtDef) -> Result<Layout, LayoutError> {
        let mut union_size = Size::ZERO;
        let mut union_align = Align::ONE;
        let mut offsets = IndexVec::new();

        for field in adt_def.fields.iter() {
            let field_layout = self.layout_of(field.ty)?;
            union_align = union_align.max(field_layout.align);
            union_size = union_size.max(field_layout.size);
            offsets.push(Size::ZERO); // All union fields at offset 0
        }

        let size = union_size.align_to(union_align);

        Ok(Layout {
            size,
            align: union_align,
            fields: FieldsShape::Arbitrary { offsets },
            variants: VariantsShape::Single { index: 0 },
            is_unsized: false,
        })
    }

    /// Compute layout for an enum ADT.
    fn layout_enum(&self, adt_def: &AdtDef, outer_ty: Ty) -> Result<Layout, LayoutError> {
        let variant_count = adt_def.variants.len();
        if variant_count == 0 {
            return Err(LayoutError::UnknownType(outer_ty));
        }

        if variant_count == 1 {
            // Single-variant enum is just a struct
            return self.layout_single_variant_enum(adt_def);
        }

        // Compute each variant's data layout (fields only, no tag)
        let variant_layouts: Vec<Layout> = adt_def
            .variants
            .iter()
            .map(|variant| self.layout_variant_data(variant))
            .collect::<Result<Vec<_>, LayoutError>>()?;

        // Try niche encoding first
        if let Some(result) = self.try_niche_encoding(adt_def, &variant_layouts)? {
            return Ok(result);
        }

        // Fall back to direct tag encoding
        self.direct_tag_encoding(adt_def, &variant_layouts)
    }

    /// Layout for a single-variant enum (degenerate case, like a struct).
    fn layout_single_variant_enum(&self, adt_def: &AdtDef) -> Result<Layout, LayoutError> {
        let variant_fields = &adt_def.variants[0].fields;
        let mut offsets = IndexVec::new();
        let mut enum_align = Align::ONE;
        let mut current_offset = Size::ZERO;

        for field in variant_fields.iter() {
            let field_layout = self.layout_of(field.ty)?;
            enum_align = enum_align.max(field_layout.align);
            current_offset = current_offset.align_to(field_layout.align);
            offsets.push(current_offset);
            current_offset = current_offset + field_layout.size;
        }

        let size = current_offset.align_to(enum_align);
        Ok(Layout {
            size,
            align: enum_align,
            fields: FieldsShape::Arbitrary { offsets },
            variants: VariantsShape::Single { index: 0 },
            is_unsized: false,
        })
    }

    /// Compute layout for a single variant's data (fields only, no tag).
    fn layout_variant_data(&self, variant: &glyim_type::adt_def::VariantDef) -> Result<Layout, LayoutError> {
        let mut offsets = IndexVec::new();
        let mut var_align = Align::ONE;
        let mut current_offset = Size::ZERO;

        for field in variant.fields.iter() {
            let field_layout = self.layout_of(field.ty)?;
            var_align = var_align.max(field_layout.align);
            current_offset = current_offset.align_to(field_layout.align);
            offsets.push(current_offset);
            current_offset = current_offset + field_layout.size;
        }

        let size = current_offset.align_to(var_align);
        Ok(Layout {
            size,
            align: var_align,
            fields: FieldsShape::Arbitrary { offsets },
            variants: VariantsShape::Single { index: 0 },
            is_unsized: false,
        })
    }

    /// Try to use niche encoding for an enum.
    fn try_niche_encoding(
        &self,
        adt_def: &AdtDef,
        variant_layouts: &[Layout],
    ) -> Result<Option<Layout>, LayoutError> {
        let variant_count = adt_def.variants.len();
        let niche_variants_needed = u128::try_from(variant_count.saturating_sub(1)).unwrap_or(u128::MAX);

        for (vi, variant) in adt_def.variants.iter().enumerate() {
            for (fi, field) in variant.fields.iter().enumerate() {
                if let Some((niche_start, niche_count)) = self.niche_info(field.ty)
                    && niche_count >= niche_variants_needed
                {
                    return Ok(Some(self.build_niche_layout(
                        variant_layouts,
                        vi,
                        fi,
                        field.ty,
                        niche_start,
                        niche_count,
                        variant_count,
                    )?));
                }
            }
        }

        Ok(None)
    }

    /// Build a niche-encoded layout for an enum.
    #[allow(clippy::too_many_arguments)]
    fn build_niche_layout(
        &self,
        variant_layouts: &[Layout],
        niche_variant_idx: usize,
        niche_field_idx: usize,
        niche_field_ty: Ty,
        niche_start: u128,
        _niche_count: u128,
        variant_count: usize,
    ) -> Result<Layout, LayoutError> {
        let data_layout = &variant_layouts[niche_variant_idx];
        let _field_offset = match &data_layout.fields {
            FieldsShape::Arbitrary { offsets } => offsets
                .get(FieldIdx::from_raw(
                    u32::try_from(niche_field_idx).unwrap_or(u32::MAX),
                ))
                .copied()
                .unwrap_or(Size::ZERO),
            FieldsShape::Primitive => Size::ZERO,
            FieldsShape::Array { .. } => Size::ZERO,
        };

        // The untagged variant is the one holding the niche field.
        // Niche variants are all OTHER variants, mapped to niche values starting at niche_start.
        let niche_variants = if niche_variant_idx == 0 {
            1..=u32::try_from(variant_count - 1).unwrap_or(u32::MAX)
        } else {
            0..=u32::try_from(niche_variant_idx - 1).unwrap_or(0)
        };

        // Size = max of all variant data sizes (they share the niche field's space)
        let mut max_size = Size::ZERO;
        let mut max_align = Align::ONE;
        for vl in variant_layouts {
            max_size = max_size.max(vl.size);
            max_align = max_align.max(vl.align);
        }
        let size = max_size.align_to(max_align);

        let output_variants: Vec<Layout> = variant_layouts.to_vec();

        Ok(Layout {
            size,
            align: max_align,
            fields: FieldsShape::Arbitrary {
                offsets: match &data_layout.fields {
                    FieldsShape::Arbitrary { offsets } => offsets.clone(),
                    FieldsShape::Primitive => {
                        let mut o = IndexVec::new();
                        o.push(Size::ZERO);
                        o
                    }
                    FieldsShape::Array { stride, count } => {
                        let mut o = IndexVec::new();
                        let mut off = Size::ZERO;
                        for _ in 0..*count {
                            o.push(off);
                            off = off + *stride;
                        }
                        o
                    }
                },
            },
            variants: VariantsShape::Multiple {
                tag: niche_field_ty,
                tag_field: u32::try_from(niche_field_idx).unwrap_or(0),
                tag_encoding: TagEncoding::Niche {
                    untagged_variant: u32::try_from(niche_variant_idx).unwrap_or(0),
                    niche_variants,
                    niche_start,
                },
                variants: output_variants,
            },
            is_unsized: false,
        })
    }

    /// Returns `(niche_start, niche_count)` if the type has unused bit patterns
    /// that can be used for niche encoding.
    fn niche_info(&self, ty: Ty) -> Option<(u128, u128)> {
        match self.ctx.ty_kind(ty) {
            TyKind::Bool => {
                // bool: valid values 0..=1, niche values 2..=255
                Some((2, 254))
            }
            TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => {
                // References are non-null; null (0) is a niche value
                Some((0, 1))
            }
            TyKind::Int(int_ty) => {
                // Signed integers: the minimum value is a niche
                let bw = int_ty.bit_width(&self.target);
                match bw {
                    8 => Some((0x80, 1)),
                    16 => Some((0x8000, 1)),
                    32 => Some((0x8000_0000, 1)),
                    64 => Some((0x8000_0000_0000_0000, 1)),
                    _ => None,
                }
            }
            TyKind::Char => {
                // char: valid 0..=0x10FFFF, niche 0x110000..=0xFFFFFFFF
                Some((0x110000, u128::MAX - 0x110000 + 1))
            }
            _ => None,
        }
    }

    /// Choose the discriminant type for an enum with the given number of variants.
    /// Returns (tag_size, tag_align, tag_ty) where tag_ty is a best-effort Ty from the context.
    fn discriminant_info(&self, variant_count: usize) -> (Size, Align, Ty) {
        if variant_count <= 256 {
            (Size::bytes(1), Align::ONE, self.ctx.bool_ty())
        } else if variant_count <= 65_536 {
            (Size::bytes(2), Align::from_bytes(2), self.ctx.bool_ty())
        } else if variant_count <= 4_294_967_296 {
            (Size::bytes(4), Align::from_bytes(4), self.ctx.bool_ty())
        } else {
            (Size::bytes(8), Align::EIGHT, self.ctx.bool_ty())
        }
    }

    /// Compute direct tag encoding for an enum.
    fn direct_tag_encoding(
        &self,
        adt_def: &AdtDef,
        variant_layouts: &[Layout],
    ) -> Result<Layout, LayoutError> {
        let variant_count = adt_def.variants.len();
        let (tag_size, tag_align, tag_ty) = self.discriminant_info(variant_count);

        let mut max_variant_size = Size::ZERO;
        let mut overall_align = tag_align;

        let mut tagged_variant_layouts: Vec<Layout> = Vec::with_capacity(variant_count);

        for variant_data in variant_layouts {
            overall_align = overall_align.max(variant_data.align);

            // Tag at offset 0, variant data starts after tag (aligned)
            let data_start = tag_size.align_to(variant_data.align);
            let variant_end = data_start + variant_data.size;
            max_variant_size = max_variant_size.max(variant_end);

            tagged_variant_layouts.push(variant_data.clone());
        }

        let size = max_variant_size.align_to(overall_align);

        // Build the top-level field offsets: tag at offset 0
        let mut offsets = IndexVec::new();
        offsets.push(Size::ZERO); // tag field at offset 0

        // Variant data starts after the tag
        let data_start = tag_size.align_to(
            variant_layouts
                .iter()
                .map(|v| v.align)
                .max()
                .unwrap_or(Align::ONE),
        );
        offsets.push(data_start);

        Ok(Layout {
            size,
            align: overall_align,
            fields: FieldsShape::Arbitrary { offsets },
            variants: VariantsShape::Multiple {
                tag: tag_ty,
                tag_field: 0,
                tag_encoding: TagEncoding::Direct,
                variants: tagged_variant_layouts,
            },
            is_unsized: false,
        })
    }

    /// Compute layout for an ADT (struct, enum, or union).
    fn layout_adt(&self, adt_id: glyim_core::AdtId, substs: Substitution, outer_ty: Ty) -> Result<Layout, LayoutError> {
        let adt_def = match self.ctx.adt_def(adt_id) {
            Some(def) => def,
            None => {
                // No AdtDef registered — if there's an AdtRepr, use that as a simple field list
                if let Some(repr) = self.ctx.adt_repr(adt_id) {
                    let mut offsets = IndexVec::new();
                    let mut struct_align = Align::ONE;
                    let mut current_offset = Size::ZERO;

                    for field_ty in &repr.field_tys {
                        let field_layout = self.layout_of(*field_ty)?;
                        struct_align = struct_align.max(field_layout.align);
                        current_offset = current_offset.align_to(field_layout.align);
                        offsets.push(current_offset);
                        current_offset = current_offset + field_layout.size;
                    }

                    let size = current_offset.align_to(struct_align);
                    return Ok(Layout {
                        size,
                        align: struct_align,
                        fields: FieldsShape::Arbitrary { offsets },
                        variants: VariantsShape::Single { index: 0 },
                        is_unsized: false,
                    });
                }
                return Err(LayoutError::UnknownType(outer_ty));
            }
        };

        // Substitute generic arguments if present
        let _args = self.ctx.substitution_args(substs);
        // TODO: substitute type parameters in adt_def fields once we have
        // a substitution engine. For now, fields use concrete types.

        match adt_def.kind {
            AdtKind::Struct => self.layout_struct(adt_def),
            AdtKind::Enum => self.layout_enum(adt_def, outer_ty),
            AdtKind::Union => self.layout_union(adt_def),
        }
    }

    /// Determine the pass mode for a function argument or return value.
    fn pass_mode_for(&self, ty: Ty, layout: &Layout, conv: CallConvention) -> PassMode {
        // Never type: function diverges, ignore return
        if matches!(self.ctx.ty_kind(ty), TyKind::Never) {
            return PassMode::Ignore;
        }

        // Unit type: no return value
        if matches!(self.ctx.ty_kind(ty), TyKind::Unit) {
            return PassMode::Ignore;
        }

        // ZST (zero-sized type): ignore unless it has non-trivial drop
        if layout.size == Size::ZERO && !layout.is_unsized {
            return PassMode::Ignore;
        }

        // On C/SystemV/AAPCS calling conventions, large types are passed indirectly
        match conv {
            CallConvention::C | CallConvention::System => {
                // On both SystemV (x86_64) and AAPCS (aarch64), aggregates larger
                // than 16 bytes are passed indirectly.
                if layout.size.0 > 16 {
                    return PassMode::Indirect { meta_attrs: false };
                }
                PassMode::Direct
            }
            CallConvention::Glyim => PassMode::Direct,
        }
    }
}

impl LayoutComputer for SimpleLayoutComputer<'_> {
    fn layout_of(&self, ty: Ty) -> Result<Layout, LayoutError> {
        let ptr_size = Size::bytes(self.target.pointer_size());
        let ptr_align = Align::from_bytes(self.target.pointer_align());

        let layout = match self.ctx.ty_kind(ty) {
            // Primitives
            TyKind::Bool => Layout::scalar(Size::bytes(1), Align::ONE),
            TyKind::Int(i) => {
                let bw = i.bit_width(&self.target);
                let byte_width = bw as u64 / 8;
                Layout::scalar(Size::bytes(byte_width), Align::from_bytes(byte_width))
            }
            TyKind::Uint(u) => {
                let bw = u.bit_width(&self.target);
                let byte_width = bw as u64 / 8;
                Layout::scalar(Size::bytes(byte_width), Align::from_bytes(byte_width))
            }
            TyKind::Float(f) => {
                let bw = f.bit_width();
                let byte_width = bw as u64 / 8;
                Layout::scalar(Size::bytes(byte_width), Align::from_bytes(byte_width))
            }
            TyKind::Char => Layout::scalar(Size::bytes(4), Align::from_bytes(4)),
            TyKind::Never => Layout::scalar(Size::ZERO, Align::ONE),
            TyKind::Unit => Layout::unit(),

            // Pointer types
            TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => Layout::scalar(ptr_size, ptr_align),
            TyKind::FnPtr(_) => Layout::scalar(ptr_size, ptr_align),

            // Dynamic (fat pointer: data + vtable)
            TyKind::Dynamic(_binder, _region) => {
                let raw_size = self.target.pointer_size();
                Layout {
                    size: Size::bytes(raw_size.saturating_mul(2)),
                    align: ptr_align,
                    fields: FieldsShape::Arbitrary {
                        offsets: {
                            let mut off = IndexVec::new();
                            off.push(Size::ZERO);
                            off.push(Size::bytes(raw_size));
                            off
                        },
                    },
                    variants: VariantsShape::Single { index: 0 },
                    is_unsized: false,
                }
            }

            // Unsized types
            TyKind::Slice(_) => return Err(LayoutError::Unsized(ty)),

            // Tuples
            TyKind::Tuple(substs) => return self.layout_tuple(*substs),

            // Arrays
            TyKind::Array(inner, count) => return self.layout_array(*inner, count, ty),

            // ADTs
            TyKind::Adt(adt_id, substs) => return self.layout_adt(*adt_id, *substs, ty),

            // Inference variables — cannot compute layout
            TyKind::Infer(_) => return Err(LayoutError::UnknownType(ty)),

            // Error type
            TyKind::Error => return Err(LayoutError::UnknownType(ty)),

            // Placeholder, bound, param types — cannot compute layout without substitution
            TyKind::Param(_) | TyKind::Bound(_, _) => return Err(LayoutError::UnknownType(ty)),

            // Opaque types — cannot compute layout
            TyKind::Opaque(_, _) => return Err(LayoutError::UnknownType(ty)),

            // Projection types — cannot compute layout
            TyKind::Projection(_) => return Err(LayoutError::UnknownType(ty)),

            // Closures — treat as unknown for now
            TyKind::Closure(_, _) => return Err(LayoutError::UnknownType(ty)),

            // String type — unsized
            TyKind::String => return Err(LayoutError::Unsized(ty)),

            // Function definitions are not types with layout
            TyKind::FnDef(_, _) => return Err(LayoutError::UnknownType(ty)),
        };

        if layout.align.0 > ALIGN_MAX {
            return Err(LayoutError::AlignmentExceedsRuntime {
                ty,
                align: layout.align.0,
                max: ALIGN_MAX,
            });
        }

        Ok(layout)
    }

    fn fn_abi_of(&self, sig: &FnSig) -> Result<FnAbi, LayoutError> {
        let conv = CallConvention::from(sig.abi);
        let args = self.ctx.substitution_args(sig.inputs);

        let arg_abis: Vec<ArgAbi> = args
            .iter()
            .filter_map(|arg| {
                if let GenericArg::Ty(t) = arg {
                    let layout = self.layout_of(*t).ok()?;
                    let mode = self.pass_mode_for(*t, &layout, conv);
                    Some(ArgAbi {
                        ty: *t,
                        layout,
                        mode,
                    })
                } else {
                    None
                }
            })
            .collect();

        let ret_layout = self.layout_of(sig.output)?;
        let ret_mode = self.pass_mode_for(sig.output, &ret_layout, conv);

        Ok(FnAbi {
            args: arg_abis,
            ret: ArgAbi {
                ty: sig.output,
                layout: ret_layout,
                mode: ret_mode,
            },
            conv,
            c_variadic: sig.c_variadic,
        })
    }

    fn ptr_size(&self) -> Size {
        Size::bytes(self.target.pointer_size())
    }
    fn ptr_align(&self) -> Align {
        Align::from_bytes(self.target.pointer_align())
    }
    fn target_info(&self) -> &TargetInfo {
        &self.target
    }
}

#[cfg(test)]
mod tests;

pub mod vtable;

impl crate::vtable::VTableComputer for SimpleLayoutComputer<'_> {
    fn vtable_of(
        &self,
        trait_def_id: glyim_core::TraitDefId,
        concrete_ty: glyim_type::Ty,
    ) -> Option<crate::vtable::VTableLayout> {
        let concrete_layout = self.layout_of(concrete_ty).ok()?;
        Some(crate::vtable::VTableLayout {
            trait_def_id,
            concrete_ty,
            size: concrete_layout.size,
            align: concrete_layout.align,
            drop_fn: None,
            methods: vec![],
        })
    }
}
