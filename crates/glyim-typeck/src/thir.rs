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

impl Expr {
    #[inline]
    pub fn err(span: Span) -> Self {
        Self {
            kind: ExprKind::Err,
            ty: Ty::ERROR,
            span,
        }
    }
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

impl Pattern {
    #[inline]
    pub fn wild(ty: Ty, span: Span) -> Self {
        Self {
            kind: PatternKind::Wild,
            ty,
            span,
        }
    }

    #[inline]
    pub fn binding(name: Name, mutability: Mutability, ty: Ty, span: Span) -> Self {
        Self {
            kind: PatternKind::Binding {
                name,
                mutability,
                subpattern: None,
            },
            ty,
            span,
        }
    }

    #[inline]
    pub fn err(span: Span) -> Self {
        Self {
            kind: PatternKind::Error,
            ty: Ty::ERROR,
            span,
        }
    }
}

// Manual implementation of Clone and Debug to avoid derive issues
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
    Range {
        start: Option<Literal>,
        end: Option<Literal>,
        inclusive: bool,
    },
    ConstBlock(Box<Body>),
    Error,
}

// Manual Debug for PatternKind to avoid derive non-exhaustive error
impl std::fmt::Debug for PatternKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternKind::Wild => write!(f, "Wild"),
            PatternKind::Binding {
                name,
                mutability,
                subpattern,
            } => f
                .debug_struct("Binding")
                .field("name", name)
                .field("mutability", mutability)
                .field("subpattern", subpattern)
                .finish(),
            PatternKind::Struct {
                adt_id,
                variant_idx,
                fields,
                rest,
            } => f
                .debug_struct("Struct")
                .field("adt_id", adt_id)
                .field("variant_idx", variant_idx)
                .field("fields", fields)
                .field("rest", rest)
                .finish(),
            PatternKind::Tuple(pats) => f.debug_tuple("Tuple").field(pats).finish(),
            PatternKind::Or(pats) => f.debug_tuple("Or").field(pats).finish(),
            PatternKind::Literal(lit) => f.debug_tuple("Literal").field(lit).finish(),
            PatternKind::Range {
                start,
                end,
                inclusive,
            } => f
                .debug_struct("Range")
                .field("start", start)
                .field("end", end)
                .field("inclusive", inclusive)
                .finish(),
            PatternKind::ConstBlock(body) => f.debug_tuple("ConstBlock").field(body).finish(),
            PatternKind::Error => write!(f, "Error"),
        }
    }
}

// Manual Clone for PatternKind
impl Clone for PatternKind {
    fn clone(&self) -> Self {
        match self {
            PatternKind::Wild => PatternKind::Wild,
            PatternKind::Binding {
                name,
                mutability,
                subpattern,
            } => PatternKind::Binding {
                name: *name,
                mutability: *mutability,
                subpattern: subpattern.clone(),
            },
            PatternKind::Struct {
                adt_id,
                variant_idx,
                fields,
                rest,
            } => PatternKind::Struct {
                adt_id: *adt_id,
                variant_idx: *variant_idx,
                fields: fields.clone(),
                rest: *rest,
            },
            PatternKind::Tuple(pats) => PatternKind::Tuple(pats.clone()),
            PatternKind::Or(pats) => PatternKind::Or(pats.clone()),
            PatternKind::Literal(lit) => PatternKind::Literal(lit.clone()),
            PatternKind::Range {
                start,
                end,
                inclusive,
            } => PatternKind::Range {
                start: start.clone(),
                end: end.clone(),
                inclusive: *inclusive,
            },
            PatternKind::ConstBlock(body) => PatternKind::ConstBlock(body.clone()),
            PatternKind::Error => PatternKind::Error,
        }
    }
}
