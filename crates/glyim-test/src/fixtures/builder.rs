use glyim_core::interner::Interner;
use glyim_core::primitives::*;
use glyim_type::{Region, Ty, TyCtx, TyCtxMut, TyKind};

pub struct SourceBuilder {
    lines: Vec<String>,
}

impl SourceBuilder {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }
    pub fn line(mut self, line: impl Into<String>) -> Self {
        self.lines.push(line.into());
        self
    }
    pub fn empty(self) -> Self {
        self.line("")
    }
    pub fn fn_def(self, name: &str, params: &str, body: &str) -> Self {
        self.line(format!("fn {}({}) {{ {} }}", name, params, body))
    }
    pub fn mode(self, mode: &str) -> Self {
        self.line(format!("// test-mode: {}", mode))
    }
    pub fn annotation(self, ann: &str) -> Self {
        self.line(format!("//~ {}", ann))
    }
    pub fn build(self) -> String {
        self.lines.join("\n")
    }
}

impl Default for SourceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TyCtxBuilder {
    interner: Option<Interner>,
}

impl TyCtxBuilder {
    pub fn new() -> Self {
        Self { interner: None }
    }
    pub fn with_interner(mut self, interner: Interner) -> Self {
        self.interner = Some(interner);
        self
    }
    pub fn build_mut(self) -> TyCtxMut {
        let interner = self.interner.unwrap_or_default();
        TyCtxMut::new(interner)
    }
    pub fn build(self) -> TyCtx {
        self.build_mut().freeze()
    }
}

impl Default for TyCtxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TyFactory;

impl TyFactory {
    pub fn bool(ctx: &mut TyCtxMut) -> Ty {
        ctx.bool_ty()
    }
    pub fn never(ctx: &mut TyCtxMut) -> Ty {
        ctx.never_ty()
    }
    pub fn unit(ctx: &mut TyCtxMut) -> Ty {
        ctx.unit_ty()
    }
    pub fn i32(ctx: &mut TyCtxMut) -> Ty {
        ctx.mk_ty(TyKind::Int(IntTy::I32))
    }
    pub fn u32(ctx: &mut TyCtxMut) -> Ty {
        ctx.mk_ty(TyKind::Uint(UintTy::U32))
    }
    pub fn f64(ctx: &mut TyCtxMut) -> Ty {
        ctx.mk_ty(TyKind::Float(FloatTy::F64))
    }
    pub fn ref_to(ctx: &mut TyCtxMut, inner: Ty, mutability: Mutability) -> Ty {
        ctx.mk_ref(Region::Erased, inner, mutability)
    }
    pub fn slice_of(ctx: &mut TyCtxMut, inner: Ty) -> Ty {
        ctx.mk_ty(TyKind::Slice(inner))
    }
}
