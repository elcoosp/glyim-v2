//! Shared syntax kind enum, Rowan language definition, CST types.
//!
//! [F8] `try_from_raw` uses `num_enum::TryFromPrimitive` derive.

pub use glyim_core::primitives::{BinOp, UnOp};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, num_enum::TryFromPrimitive)]
#[repr(u16)]
pub enum SyntaxKind {
    // Keywords
    KwFn,
    KwLet,
    KwStruct,
    KwEnum,
    KwIf,
    KwElse,
    KwReturn,
    KwMatch,
    KwMod,
    KwComptime,
    KwSelf,
    KwSuper,
    KwCrate,
    KwTrue,
    KwFalse,
    KwMut,
    KwRef,
    KwAs,
    KwWhile,
    KwFor,
    KwLoop,
    KwIn,
    KwBreak,
    KwContinue,
    KwTrait,
    KwImpl,
    KwWhere,
    KwType,
    KwPub,
    KwPriv,
    KwExtern,
    KwUnsafe,
    KwConst,
    KwStatic,
    KwMove,
    // Literals
    IntLit,
    FloatLit,
    StringLit,
    CharLit,
    BoolLit,
    Ident,
    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    Bang,
    BangEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
    AndAnd,
    OrOr,
    Caret,
    Shl,
    Shr,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    // Punctuation
    Arrow,
    FatArrow,
    Dot,
    DotDot,
    DotDotEq,
    Comma,
    Semicolon,
    Colon,
    ColonColon,
    At,
    Hash,
    Dollar,
    Tilde,
    Underscore,
    Question,
    // Delimiters
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    // Trivia
    Whitespace,
    LineComment,
    BlockComment,
    DocComment,
    // Nodes
    SourceFile,
    Module,
    FnDef,
    StructDef,
    EnumDef,
    TraitDef,
    ImplDef,
    TypeAlias,
    ConstDef,
    StaticDef,
    UseDecl,
    ExternBlock,
    ParamList,
    Param,
    TypeParamList,
    TypeParam,
    WhereClause,
    Block,
    LetStmt,
    ExprStmt,
    IfExpr,
    WhileExpr,
    LoopExpr,
    ForExpr,
    MatchExpr,
    MatchArmList,
    MatchArm,
    CallExpr,
    MethodCallExpr,
    FieldExpr,
    IndexExpr,
    UnaryExpr,
    BinaryExpr,
    CastExpr,
    RefExpr,
    ClosureExpr,
    PathExpr,
    LitExpr,
    ArrayExpr,
    TupleExpr,
    StructExpr,
    RangeExpr,
    BreakExpr,
    ContinueExpr,
    AssignExpr,
    PathType,
    FnType,
    RefType,
    SliceType,
    ArrayType,
    TupleType,
    NeverType,
    InferType,
    GenericArgList,
    PatIdent,
    PatStruct,
    PatTuple,
    PatOr,
    PatLit,
    PatWild,
    UsePath,
    UseTree,
    MacroCall,
    TokenTree,
    StructField,
    EnumVariant,
    FieldList,
    VariantList,
    // Error
    Error,
}

impl SyntaxKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment | Self::DocComment
        )
    }
    pub fn is_keyword(self) -> bool {
        (self as u16) >= Self::KwFn as u16 && (self as u16) <= Self::KwStatic as u16
    }
    pub fn is_literal(self) -> bool {
        matches!(
            self,
            Self::IntLit | Self::FloatLit | Self::StringLit | Self::CharLit | Self::BoolLit
        )
    }
    pub fn is_node(self) -> bool {
        (self as u16) >= Self::SourceFile as u16 && (self as u16) < Self::Error as u16
    }

    pub fn try_from_raw(raw: u16) -> Option<Self> {
        Self::try_from(raw).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GlyimLang {}

impl rowan::Language for GlyimLang {
    type Kind = SyntaxKind;
    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        Self::Kind::try_from_raw(raw.0).unwrap_or(Self::Kind::Error)
    }
    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind as u16)
    }
}

pub type SyntaxNode = rowan::SyntaxNode<GlyimLang>;
pub type SyntaxToken = rowan::SyntaxToken<GlyimLang>;
pub type SyntaxElement = rowan::SyntaxElement<GlyimLang>;
pub type GreenNode = rowan::GreenNode;
pub type GreenToken = rowan::GreenToken;

pub trait AstNode {
    fn can_cast(kind: SyntaxKind) -> bool;
    fn cast(node: SyntaxNode) -> Option<Self>
    where
        Self: Sized;
    fn syntax(&self) -> &SyntaxNode;
}

pub fn child_of_kind(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|c| c.kind() == kind)
}

macro_rules! ast_node {
    ($name:ident, $kind:expr) => {
        pub struct $name(SyntaxNode);
        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                kind == $kind
            }
            fn cast(node: SyntaxNode) -> Option<Self> {
                if node.kind() == $kind {
                    Some(Self(node))
                } else {
                    None
                }
            }
            fn syntax(&self) -> &SyntaxNode {
                &self.0
            }
        }
    };
}

ast_node!(SourceFile, SyntaxKind::SourceFile);
ast_node!(FnDef, SyntaxKind::FnDef);
ast_node!(StructDef, SyntaxKind::StructDef);
ast_node!(EnumDef, SyntaxKind::EnumDef);
ast_node!(TraitDef, SyntaxKind::TraitDef);
ast_node!(ImplDef, SyntaxKind::ImplDef);
ast_node!(Block, SyntaxKind::Block);
ast_node!(CallExpr, SyntaxKind::CallExpr);
ast_node!(BinaryExpr, SyntaxKind::BinaryExpr);
ast_node!(PathExpr, SyntaxKind::PathExpr);
ast_node!(LitExpr, SyntaxKind::LitExpr);
