//! Typed High-Level IR — fully typed, still generic.

use glyim_core::def_id::{AdtId, DefId, FnDefId};
use glyim_core::interner::Name;
use glyim_core::primitives::*;
use glyim_span::Span;
use glyim_type::*;

glyim_core::define_idx!(LocalVarId);

#[derive(Clone, Debug)]
pub struct Body {
    pub owner: DefId,
    pub params: Vec<Param>,
    pub return_ty: Ty,
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: Name,
    pub ty: Ty,
    pub span: Span,
    pub pat: Pattern,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let {
        name: Name,
        ty: Ty,
        pat: Pattern,
        init: Option<Expr>,
        span: Span,
    },
    Assign {
        lhs: Expr,
        rhs: Expr,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Expr {
        expr: Expr,
    },
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Literal(Literal),
    VarRef(LocalVarId),
    FnRef(FnDefId),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
    },
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Block {
        stmts: Vec<Stmt>,
        tail: Option<Box<Expr>>,
    },
    Ref {
        mutability: Mutability,
        operand: Box<Expr>,
    },
    Field {
        receiver: Box<Expr>,
        field: Name,
        ty: Ty,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Cast {
        expr: Box<Expr>,
    },
    While {
        cond: Box<Expr>,
        body: Box<Expr>,
    },
    Loop {
        body: Box<Expr>,
    },
    For {
        pat: Box<Pattern>,
        iterable: Box<Expr>,
        body: Box<Expr>,
    },
    Array(Vec<Expr>),
    Tuple(Vec<Expr>),
    Struct {
        adt_id: AdtId,
        variant_idx: u32,
        fields: Vec<(Name, Expr)>,
        spread: Option<Box<Expr>>,
    },
    Break {
        value: Option<Box<Expr>>,
    },
    Continue,
    Closure {
        body: Box<Body>,
        captures: Vec<Capture>,
    },
    Err,
}

#[derive(Clone, Debug)]
pub struct MatchArm {
    pub pat: Pattern,
    pub guard: Option<Box<Expr>>,
    pub body: Expr,
}

#[derive(Clone, Debug)]
pub struct Pattern {
    pub kind: PatternKind,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum PatternKind {
    Wild,
    Binding {
        name: Name,
        mutability: Mutability,
        subpattern: Option<Box<Pattern>>,
    },
    Struct {
        adt_id: AdtId,
        variant_idx: u32,
        fields: Vec<FieldPat>,
        rest: bool,
    },
    Tuple(Vec<Pattern>),
    Or(Vec<Pattern>),
    Literal(Literal),
    ConstBlock(Box<Body>),
    Error,
}

#[derive(Clone, Debug)]
pub struct FieldPat {
    pub field: Name,
    pub pattern: Pattern,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Capture {
    pub local: LocalVarId,
    pub kind: CaptureKind,
    pub ty: Ty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureKind {
    ByValue,
    ByRef(Mutability),
}

#[derive(Clone, Debug)]
pub enum Literal {
    Int(i128, Option<IntTy>),
    Uint(u128, Option<UintTy>),
    FloatBits(u64, FloatTy),
    Bool(bool),
    Char(char),
    String(Name),
    Unit,
}
