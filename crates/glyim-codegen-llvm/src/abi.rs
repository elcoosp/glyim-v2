use glyim_core::TargetInfo;
use glyim_layout::{
    Align, ArgAbi, CallConvention, FieldsShape, FnAbi, Layout, LayoutComputer, LayoutError,
    PassMode, SimpleLayoutComputer, Size, VariantsShape,
};
use glyim_type::{ConstKind, Ty, TyCtx, TyKind};

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
                let mut field_layouts = Vec::new();
                for arg in args {
                    if let glyim_type::GenericArg::Ty(t) = arg {
                        field_layouts.push(self.layout_of(*t)?);
                    }
                }
                if field_layouts.is_empty() {
                    return Ok(Layout::unit());
                }
                let mut size = Size::ZERO;
                let mut align = Align::ONE;
                let mut offsets = glyim_core::arena::IndexVec::new();
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
        let ptr_size = self.ptr_size();
        let large_threshold = ptr_size.0 * 2; // > 2*ptr_size -> indirect

        // Classify return type
        let ret_layout = self.layout_of(sig.output)?;
        let ret_mode = if ret_layout.size.0 == 0 {
            PassMode::Ignore
        } else if ret_layout.size.0 > large_threshold {
            PassMode::Indirect { meta_attrs: false }
        } else {
            PassMode::Direct
        };

        // Classify arguments
        let args = self.ctx.substitution_args(sig.inputs);
        let mut arg_abis = Vec::new();
        for arg in args {
            if let glyim_type::GenericArg::Ty(t) = arg {
                let layout = self.layout_of(*t)?;
                let mode = if layout.size.0 == 0 {
                    PassMode::Ignore
                } else if layout.size.0 > large_threshold {
                    PassMode::Indirect { meta_attrs: false }
                } else {
                    PassMode::Direct
                };
                arg_abis.push(ArgAbi {
                    ty: *t,
                    layout,
                    mode,
                });
            }
        }

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
