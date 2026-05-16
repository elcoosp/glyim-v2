use glyim_core::TargetInfo;
use glyim_layout::{
    Align, ArgAbi, CallConvention, FieldsShape, FnAbi, Layout, LayoutComputer, LayoutError,
    PassMode, SimpleLayoutComputer, Size, VariantsShape,
};
use glyim_type::{ConstKind, FieldIdx, Ty, TyCtx, TyKind};

pub(crate) struct FullLayoutComputer<'a> {
    simple: SimpleLayoutComputer<'a>,
    ctx: &'a TyCtx,
}

impl<'a> FullLayoutComputer<'a> {
    pub fn new(ctx: &'a TyCtx, target: TargetInfo) -> Self {
        Self {
            simple: SimpleLayoutComputer::new(ctx, target),
            ctx,
        }
    }

    fn ptr_size(&self) -> Size {
        self.simple.ptr_size()
    }
}

impl LayoutComputer for FullLayoutComputer<'_> {
    fn layout_of(&self, ty: Ty) -> Result<Layout, LayoutError> {
        match self.ctx.ty_kind(ty) {
            TyKind::Tuple(subst) => {
                let args = self.ctx.substitution_args(*subst);
                let mut field_layouts: Vec<Layout> = Vec::new();
                for arg in args {
                    if let glyim_type::GenericArg::Ty(t) = arg {
                        field_layouts.push(self.layout_of(*t)?);
                    } else {
                        return Err(LayoutError::UnknownType(ty));
                    }
                }
                if field_layouts.is_empty() {
                    return Ok(Layout::unit());
                }
                let mut size = Size::ZERO;
                let mut align = Align::ONE;
                let mut offsets: glyim_core::arena::IndexVec<FieldIdx, Size> =
                    glyim_core::arena::IndexVec::new();
                for layout in &field_layouts {
                    let offset = size.align_to(layout.align);
                    offsets.push(offset);
                    size = offset + layout.size;
                    align = align.max(layout.align);
                }
                size = size.align_to(align);
                Ok(Layout {
                    size,
                    align,
                    fields: FieldsShape::Arbitrary { offsets },
                    variants: VariantsShape::Single { index: 0 },
                    is_unsized: false,
                })
            }
            TyKind::Array(elem, count) => {
                let elem_ty = *elem;
                let count = match &count.kind {
                    ConstKind::Uint(n) => *n as u64,
                    ConstKind::Int(n) => *n as u64,
                    _ => return Err(LayoutError::UnknownType(ty)),
                };
                let elem_layout = self.layout_of(elem_ty)?;
                let stride = elem_layout.size.align_to(elem_layout.align);
                let size = Size(stride.0 * count);
                Ok(Layout {
                    size,
                    align: elem_layout.align,
                    fields: FieldsShape::Array { stride, count },
                    variants: VariantsShape::Single { index: 0 },
                    is_unsized: false,
                })
            }
            _ => self.simple.layout_of(ty),
        }
    }

    fn fn_abi_of(&self, sig: &glyim_type::FnSig) -> Result<FnAbi, LayoutError> {
        let args = self.ctx.substitution_args(sig.inputs);
        let arg_abis: Vec<ArgAbi> = args
            .iter()
            .filter_map(|arg| {
                if let glyim_type::GenericArg::Ty(t) = arg {
                    let layout = self.layout_of(*t).ok()?;
                    let mode = classify_pass_mode(&layout, self.ptr_size());
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
        let ret_mode = classify_pass_mode(&ret_layout, self.ptr_size());
        Ok(FnAbi {
            args: arg_abis,
            ret: ArgAbi {
                ty: sig.output,
                layout: ret_layout,
                mode: ret_mode,
            },
            conv: CallConvention::from(sig.abi),
            c_variadic: sig.c_variadic,
        })
    }

    fn ptr_size(&self) -> Size {
        self.simple.ptr_size()
    }

    fn ptr_align(&self) -> Align {
        self.simple.ptr_align()
    }

    fn target_info(&self) -> &TargetInfo {
        self.simple.target_info()
    }
}

fn classify_pass_mode(layout: &Layout, ptr_size: Size) -> PassMode {
    if layout.size.0 == 0 {
        PassMode::Ignore
    } else if layout.size.0 > ptr_size.0 * 2 {
        PassMode::Indirect { meta_attrs: false }
    } else {
        PassMode::Direct
    }
}
