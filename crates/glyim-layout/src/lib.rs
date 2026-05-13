//! Type layout computation — sizes, alignments, ABI details.

use glyim_core::arena::IndexVec;
use glyim_core::abi::ALIGN_MAX;
use glyim_core::primitives::{Abi, TargetInfo};
use glyim_type::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Size(pub u64);

impl Size {
    pub const ZERO: Size = Size(0);
    pub fn bytes(b: u64) -> Self { Size(b) }
    pub fn bits(&self) -> u64 { self.0 * 8 }
    pub fn align_to(&self, align: Align) -> Self {
        let mask = align.0 - 1;
        Size((self.0 + mask) & !mask)
    }
}

impl std::ops::Add for Size {
    type Output = Size;
    fn add(self, rhs: Size) -> Size { Size(self.0 + rhs.0) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Align(pub u64);

impl Align {
    pub const ONE: Align = Align(1);
    pub const EIGHT: Align = Align(8);
    pub fn from_bytes(bytes: u64) -> Self { debug_assert!(bytes.is_power_of_two()); Align(bytes) }
    pub fn max(self, other: Self) -> Self { Align(self.0.max(other.0)) }
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
        Self { size, align, fields: FieldsShape::Primitive, variants: VariantsShape::Single { index: 0 }, is_unsized: false }
    }
    pub fn unit() -> Self {
        Self { size: Size::ZERO, align: Align::ONE, fields: FieldsShape::Arbitrary { offsets: IndexVec::new() }, variants: VariantsShape::Single { index: 0 }, is_unsized: false }
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
    Single { index: u32 },
    Multiple { tag: Ty, tag_field: u32, tag_encoding: TagEncoding, variants: Vec<Layout> },
}

#[derive(Clone, Debug)]
pub enum TagEncoding {
    Direct,
    Niche { untagged_variant: u32, niche_variants: std::ops::RangeInclusive<u32>, niche_start: u128 },
}

#[derive(Clone, Debug)]
pub struct FnAbi {
    pub args: Vec<ArgAbi>,
    pub ret: ArgAbi,
    pub conv: CallConvention,
    pub c_variadic: bool,
}

#[derive(Clone, Debug)]
pub struct ArgAbi { pub ty: Ty, pub layout: Layout, pub mode: PassMode }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassMode { Direct, Indirect { meta_attrs: bool }, Ignore }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallConvention { Glyim, C, System }

impl From<Abi> for CallConvention {
    fn from(abi: Abi) -> Self {
        match abi { Abi::C => CallConvention::C, Abi::Glyim => CallConvention::Glyim, Abi::System => CallConvention::System }
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
        assert!(ALIGN_MAX >= 8, "ALIGN_MAX must be at least 8");
        Self { ctx, target }
    }
}

impl LayoutComputer for SimpleLayoutComputer<'_> {
    fn layout_of(&self, ty: Ty) -> Result<Layout, LayoutError> {
        let ptr_size = Size::bytes(self.target.pointer_size());
        let ptr_align = Align::from_bytes(self.target.pointer_align());

        let layout = match self.ctx.ty_kind(ty) {
            TyKind::Bool => Layout::scalar(Size::bytes(1), Align::ONE),
            TyKind::Int(i) => {
                let bw = i.bit_width(&self.target);
                Layout::scalar(Size::bytes(bw as u64 / 8), Align::from_bytes(bw as u64 / 8))
            }
            TyKind::Uint(u) => {
                let bw = u.bit_width(&self.target);
                Layout::scalar(Size::bytes(bw as u64 / 8), Align::from_bytes(bw as u64 / 8))
            }
            TyKind::Float(f) => {
                let bw = f.bit_width();
                Layout::scalar(Size::bytes(bw as u64 / 8), Align::from_bytes(bw as u64 / 8))
            }
            TyKind::Char => Layout::scalar(Size::bytes(4), Align::from_bytes(4)),
            TyKind::Never => Layout::scalar(Size::ZERO, Align::ONE),
            TyKind::Unit => Layout::unit(),
            TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => Layout::scalar(ptr_size, ptr_align),
            TyKind::Slice(_) | TyKind::Dynamic(_, _) => return Err(LayoutError::Unsized(ty)),
            TyKind::Error => return Err(LayoutError::UnknownType(ty)),
            _ => return Err(LayoutError::UnknownType(ty)),
        };

        if layout.align.0 > ALIGN_MAX {
            return Err(LayoutError::AlignmentExceedsRuntime { ty, align: layout.align.0, max: ALIGN_MAX });
        }

        Ok(layout)
    }

    fn fn_abi_of(&self, sig: &FnSig) -> Result<FnAbi, LayoutError> {
        let args = self.ctx.substitution_args(sig.inputs);
        let arg_abis: Vec<ArgAbi> = args.iter()
            .filter_map(|arg| {
                if let GenericArg::Ty(t) = arg {
                    Some(ArgAbi { ty: *t, layout: self.layout_of(*t).ok()?, mode: PassMode::Direct })
                } else { None }
            })
            .collect();
        let ret_layout = self.layout_of(sig.output)?;
        Ok(FnAbi {
            args: arg_abis,
            ret: ArgAbi { ty: sig.output, layout: ret_layout, mode: PassMode::Direct },
            conv: CallConvention::from(sig.abi),
            c_variadic: sig.c_variadic,
        })
    }

    fn ptr_size(&self) -> Size { Size::bytes(self.target.pointer_size()) }
    fn ptr_align(&self) -> Align { Align::from_bytes(self.target.pointer_align()) }
    fn target_info(&self) -> &TargetInfo { &self.target }
}
