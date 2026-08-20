//! High-Level IR — name-resolved, untyped.
//!
//! This crate depends only on `glyim-core` and `glyim-span`,
//! NOT on `glyim-type`. This ensures the frontend does not pull
//! in the heavy type system machinery.
//!
//! [F12] Uses `glyim_core::path::PathKind` (shared). Defines its
//! own `PathSegment` with `generic_args: Option<Vec<TypeRef>>`,
//! which differs from `glyim_core::path::PathSegment` (name-only).
#![allow(missing_docs)]
// Stylistic clippy lints suppressed crate-wide (test-noise lints).
#![allow(
    clippy::cloned_ref_to_slice_refs,
    clippy::vec_init_then_push,
    clippy::assertions_on_constants,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::manual_c_str_literals,
    clippy::doc_lazy_continuation,
    clippy::empty_line_after_doc_comments,
    clippy::manual_strip,
    clippy::needless_range_loop,
    clippy::unnecessary_cast,
    clippy::clone_on_copy,
    clippy::mutable_key_type,
    clippy::only_used_in_recursion,
    clippy::let_unit_value,
    clippy::unnecessary_literal_unwrap,
    clippy::format_in_format_args,
    clippy::permissions_set_readonly_false,
    clippy::needless_lifetimes,
    clippy::collapsible_if
)]

use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::interner::Name;
use glyim_core::path::PathKind;
use glyim_core::primitives::*;
use glyim_span::Span;

glyim_core::define_idx!(ExprId);
glyim_core::define_idx!(PatId);
glyim_core::define_idx!(BodyId);
glyim_core::define_idx!(ItemId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HirId {
    pub owner: LocalDefId,
    pub local: u32,
}

#[derive(Clone, Debug)]
pub struct CrateHir {
    pub items: IndexVec<ItemId, Item>,
    pub bodies: IndexVec<BodyId, Body>,
    pub body_owners: IndexVec<BodyId, LocalDefId>,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: ItemId,
    pub name: Name,
    pub kind: ItemKind,
    pub visibility: Visibility,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ItemKind {
    Fn(FnItem),
    Struct(StructItem),
    Enum(EnumItem),
    Trait(TraitItem),
    Impl(ImplItem),
    TypeAlias(TypeAliasItem),
    Const(ConstItem),
    Static(StaticItem),
    Mod(ModItem),
    Use(UseItem),
    Extern(ExternBlockItem),
}

#[derive(Clone, Debug)]
pub struct FnItem {
    pub params: Vec<Param>,
    pub return_ty: Option<TypeRef>,
    pub body: Option<BodyId>,
    pub is_unsafe: bool,
    pub is_async: bool,
    pub is_const: bool,
    pub generic_params: Vec<GenericParam>,
    pub where_clauses: Vec<crate::where_clause::WhereClause>,
    /// Calling convention for FFI (`extern "C" fn`), if declared. `None`
    /// means the default Glyim ABI (unstub-5 Phase 4).
    pub abi: Option<Name>,
}

#[derive(Clone, Debug)]
pub struct StructItem {
    pub fields: Vec<Field>,
    pub kind: StructKind,
    pub generic_params: Vec<GenericParam>,
    pub where_clauses: Vec<crate::where_clause::WhereClause>,
}

#[derive(Clone, Debug)]
pub struct EnumItem {
    pub variants: Vec<Variant>,
    pub generic_params: Vec<GenericParam>,
    pub where_clauses: Vec<crate::where_clause::WhereClause>,
}

#[derive(Clone, Debug)]
pub struct Variant {
    pub name: Name,
    pub fields: Vec<Field>,
    pub kind: StructKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct TraitMethod {
    pub name: Name,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeRef>,
    pub default_body: Option<BodyId>,
}

#[derive(Clone, Debug)]
pub struct ImplMethod {
    pub name: Name,
    pub body: Option<BodyId>,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeRef>,
}

#[derive(Clone, Debug)]
pub struct AssociatedTy {
    pub name: Name,
    pub bounds: Vec<TypeRef>,
    pub default: Option<TypeRef>,
}

#[derive(Clone, Debug)]
pub struct TraitItem {
    pub associated_types: Vec<AssociatedTy>,
    pub methods: Vec<TraitMethod>,
    pub generic_params: Vec<GenericParam>,
    pub where_clauses: Vec<crate::where_clause::WhereClause>,
}

#[derive(Clone, Debug)]
pub struct ImplItem {
    pub trait_ref: Option<Path>,
    pub self_ty: TypeRef,
    pub methods: Vec<ImplMethod>,
    pub generic_params: Vec<GenericParam>,
    pub where_clauses: Vec<crate::where_clause::WhereClause>,
}

#[derive(Clone, Debug)]
pub struct TypeAliasItem {
    pub ty: Option<TypeRef>,
    pub generic_params: Vec<GenericParam>,
    pub where_clauses: Vec<crate::where_clause::WhereClause>,
}

#[derive(Clone, Debug)]
pub struct ConstItem {
    pub ty: TypeRef,
    pub body: Option<BodyId>,
    /// Root expression of the constant's initializer body, used by const
    /// evaluation (Part C: const value materialization) to evaluate the value.
    pub root_expr: Option<ExprId>,
}

#[derive(Clone, Debug)]
pub struct StaticItem {
    pub ty: TypeRef,
    pub body: Option<BodyId>,
    pub is_mut: bool,
}

#[derive(Clone, Debug)]
pub struct ModItem {
    pub children: Vec<ItemId>,
}

#[derive(Clone, Debug)]
pub struct UseItem {
    pub path: Path,
    pub alias: Option<Name>,
}

#[derive(Clone, Debug)]
pub struct ExternBlockItem {
    pub items: Vec<ItemId>,
    pub abi: Option<Name>,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: Name,
    pub ty: Option<TypeRef>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: Name,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct GenericParam {
    pub name: Name,
    pub kind: GenericParamKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum GenericParamKind {
    Type {
        default: Option<TypeRef>,
        /// Trait bounds declared on the param, e.g. the `MyFuture` in
        /// `fn block_on<F: MyFuture>`. Captured so typeck can solve the bound
        /// and project associated types (`F::Output`); previously dropped.
        bounds: Vec<TypeRef>,
    },
    Lifetime,
    Const {
        ty: TypeRef,
        default: Option<ConstRef>,
    },
}

#[derive(Clone, Debug)]
pub enum TypeRef {
    Path(Path),
    Fn {
        params: Vec<TypeRef>,
        ret: Option<Box<TypeRef>>,
    },
    Ref {
        inner: Box<TypeRef>,
        mutability: Mutability,
    },
    Slice(Box<TypeRef>),
    Array {
        inner: Box<TypeRef>,
        len: ConstRef,
    },
    Tuple(Vec<TypeRef>),
    Never,
    Infer,
    /// `dyn Trait` — an unsized trait object. The inner `TypeRef` is the
    /// trait (with its bounds). Lowered from `SyntaxKind::DynType` so that
    /// `dyn Trait` is distinguishable from merely naming the trait type.
    Dyn(Box<TypeRef>),
    Error,
}

#[derive(Clone, Debug)]
pub enum ConstRef {
    Literal(Literal),
    Path(Path),
    Error,
}

#[derive(Clone, Debug)]
pub struct Path {
    pub segments: Vec<PathSegment>,
    pub kind: PathKind,
}

impl Path {
    pub fn from_single(name: Name) -> Self {
        Self {
            segments: vec![PathSegment {
                name,
                generic_args: None,
            }],
            kind: PathKind::Plain,
        }
    }
    pub fn as_name(&self) -> Option<Name> {
        if self.segments.len() == 1 && self.kind == PathKind::Plain {
            Some(self.segments[0].name)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct PathSegment {
    pub name: Name,
    pub generic_args: Option<Vec<TypeRef>>,
}

#[derive(Clone, Debug)]
pub struct Body {
    pub owner: LocalDefId,
    pub exprs: IndexVec<ExprId, Expr>,
    pub pats: IndexVec<PatId, Pat>,
    pub params: Vec<PatId>,
    pub span: Span,
    pub expr_spans: IndexVec<ExprId, Span>, // Added field
}

#[derive(Clone, Debug)]
pub enum Expr {
    Missing,
    Path(Path),
    Literal(Literal),
    Block {
        stmts: Vec<ExprId>,
        tail: Option<ExprId>,
    },
    If {
        cond: ExprId,
        then_branch: ExprId,
        else_branch: Option<ExprId>,
    },
    While {
        cond: ExprId,
        body: ExprId,
    },
    Loop {
        body: ExprId,
    },
    For {
        pat: PatId,
        iterable: ExprId,
        body: ExprId,
    },
    Match {
        scrutinee: ExprId,
        arms: Vec<MatchArm>,
    },
    Call {
        func: ExprId,
        args: Vec<ExprId>,
    },
    MethodCall {
        receiver: ExprId,
        method: Name,
        args: Vec<ExprId>,
    },
    Field {
        receiver: ExprId,
        field: Name,
    },
    Index {
        base: ExprId,
        index: ExprId,
    },
    Unary {
        op: UnOp,
        expr: ExprId,
    },
    Binary {
        op: BinOp,
        lhs: ExprId,
        rhs: ExprId,
    },
    Cast {
        expr: ExprId,
        ty: TypeRef,
    },
    Ref {
        expr: ExprId,
        mutability: Mutability,
    },
    Assign {
        lhs: ExprId,
        rhs: ExprId,
    },
    Return {
        value: Option<ExprId>,
    },
    Break {
        value: Option<ExprId>,
    },
    Continue,
    Closure {
        params: Vec<PatId>,
        body: ExprId,
        is_move: bool,
    },
    Array(Vec<ExprId>),
    Tuple(Vec<ExprId>),
    /// `let <pat> = <value>` — a named-binding statement (and the only place a
    /// new local is introduced). The pattern is bound into the local
    /// environment when this is converted to THIR `Stmt::Let`.
    Let {
        pat: PatId,
        value: ExprId,
    },
    Struct {
        path: Path,
        fields: Vec<(Name, ExprId)>,
        spread: Option<ExprId>,
    },
    Range {
        start: Option<ExprId>,
        end: Option<ExprId>,
        inclusive: bool,
    },
    Err,
}

#[derive(Clone, Debug)]
pub struct MatchArm {
    pub pat: PatId,
    pub guard: Option<ExprId>,
    pub body: ExprId,
}

#[derive(Clone, Debug)]
pub enum Pat {
    Wild,
    Binding {
        name: Name,
        mutability: Mutability,
        subpattern: Option<PatId>,
    },
    Struct {
        path: Path,
        fields: Vec<(Name, PatId)>,
        rest: bool,
    },
    Tuple(Vec<PatId>),
    Slice(Vec<PatId>),
    Or(Vec<PatId>),
    Literal(Literal),
    Range {
        start: Option<Literal>,
        end: Option<Literal>,
        inclusive: bool,
    },
    Path(Path),
    Err,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Literal {
    Int(i128, Option<IntTy>),
    Uint(u128, Option<UintTy>),
    Float(u64, FloatTy),
    Bool(bool),
    Char(char),
    String(Name),
    Unit,
}

mod lower;

pub mod pipeline_api;
#[cfg(test)]
mod tests;
pub mod where_clause;

impl Body {
    /// Allocate a new expression with its span. This is the ONLY correct way
    /// to add an expression — it guarantees `exprs` and `expr_spans` stay
    /// in sync.
    #[inline]
    pub fn alloc_expr(&mut self, expr: Expr, span: Span) -> ExprId {
        debug_assert_eq!(
            self.exprs.len(),
            self.expr_spans.len(),
            "Body::alloc_expr: exprs and expr_spans out of sync before push"
        );
        let id = self.exprs.push(expr);
        self.expr_spans.push(span);
        id
    }

    /// Convenience: allocate a `Expr::Missing` placeholder.
    #[inline]
    pub fn alloc_missing(&mut self, span: Span) -> ExprId {
        self.alloc_expr(Expr::Missing, span)
    }

    /// Runtime invariant check. Call this after body construction.
    pub fn verify_invariants(&self) -> Result<(), String> {
        if self.exprs.len() != self.expr_spans.len() {
            return Err(format!(
                "Body invariant violated: exprs has {} entries but expr_spans has {}",
                self.exprs.len(),
                self.expr_spans.len()
            ));
        }
        Ok(())
    }
}
