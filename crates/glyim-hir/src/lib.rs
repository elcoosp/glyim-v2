//! High-Level IR (name-resolved, untyped)
use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::interner::Name;
use glyim_span::Span;

glyim_core::define_idx!(ExprId);
glyim_core::define_idx!(PatId);
glyim_core::define_idx!(BodyId);
glyim_core::define_idx!(ItemId);

#[derive(Clone, Debug)]
pub struct CrateHir {
    pub items: IndexVec<ItemId, Item>,
    pub bodies: IndexVec<BodyId, Body>,
    pub body_owners: IndexVec<BodyId, LocalDefId>,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub name: Name,
    pub kind: ItemKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ItemKind {
    Fn(FnItem),
    Struct(StructItem),
    Enum(EnumItem),
}

#[derive(Clone, Debug)]
pub struct FnItem {
    pub body: Option<BodyId>,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeRef>,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: Name,
    pub ty: Option<TypeRef>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct StructItem {
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: Name,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct EnumItem {
    pub variants: Vec<Variant>,
}

#[derive(Clone, Debug)]
pub struct Variant {
    pub name: Name,
    pub fields: Vec<Field>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TypeRef {
    Path(Path),
    Never,
    Error,
}

#[derive(Clone, Debug)]
pub struct Path {
    pub segments: Vec<PathSegment>,
    pub kind: PathKind,
}

#[derive(Clone, Debug)]
pub struct PathSegment {
    pub name: Name,
    pub generic_args: Option<Vec<TypeRef>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathKind {
    Plain,
    SelfPath,
    Super(u32),
    Crate,
}

#[derive(Clone, Debug)]
pub struct Body {
    pub owner: LocalDefId,
    pub exprs: IndexVec<ExprId, Expr>,
    pub pats: IndexVec<PatId, Pat>,
    pub params: Vec<PatId>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Missing,
    Path(Path),
    Literal(Literal),
    Block { stmts: Vec<ExprId>, tail: Option<ExprId> },
    Call { func: ExprId, args: Vec<ExprId> },
    Return { value: Option<ExprId> },
    Err,
}

#[derive(Clone, Debug)]
pub enum Pat {
    Wild,
    Binding { name: Name, mutability: Mutability, subpattern: Option<PatId> },
    Err,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mutability { Not, Mut }

#[derive(Clone, Debug)]
pub enum Literal {
    Int(i128, Option<IntTy>),
    Bool(bool),
    String(Name),
    Unit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntTy { I32, I64, Isize }

impl IntTy {
    pub fn name(self) -> &'static str {
        match self { Self::I32 => "i32", Self::I64 => "i64", Self::Isize => "isize" }
    }
}
