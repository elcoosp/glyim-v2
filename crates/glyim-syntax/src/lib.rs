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
    KwDyn,
    KwType,
    KwPub,
    KwPriv,
    KwExtern,
    KwUnsafe,
    KwUse,
    KwConst,
    KwStatic,
    KwMove,
    KwMacroRules,
    Lifetime,
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
    ReturnExpr,
    AssignExpr,
    PathType,
    FnType,
    DynType,
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
    PatRange,
    PatWild,
    UsePath,
    UseTree,
    MacroCall,
    TokenTree,
    MacroDef,
    MacroArm,
    MacroPattern,
    StructField,
    EnumVariant,
    FieldList,
    VariantList,
    // Error
    Error,
    // Visibility qualifiers
    Visibility,
    VisCrate,
    VisSuper,
    VisSelf,
    VisPath,
    // Where clause components
    WherePredicate,
    Bound,
    // Macro metavariables
    MetaVar,
    MetaVarCrate,
}

impl SyntaxKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment | Self::DocComment
        )
    }
    pub fn is_keyword(self) -> bool {
        (self as u16) >= Self::KwFn as u16 && (self as u16) <= Self::KwMacroRules as u16
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
ast_node!(Module, SyntaxKind::Module);
ast_node!(TypeAlias, SyntaxKind::TypeAlias);
ast_node!(ConstDef, SyntaxKind::ConstDef);
ast_node!(StaticDef, SyntaxKind::StaticDef);
ast_node!(UseDecl, SyntaxKind::UseDecl);
ast_node!(ExternBlock, SyntaxKind::ExternBlock);
ast_node!(ParamList, SyntaxKind::ParamList);
ast_node!(Param, SyntaxKind::Param);
ast_node!(TypeParamList, SyntaxKind::TypeParamList);
ast_node!(TypeParam, SyntaxKind::TypeParam);
ast_node!(WhereClause, SyntaxKind::WhereClause);
ast_node!(LetStmt, SyntaxKind::LetStmt);
ast_node!(ExprStmt, SyntaxKind::ExprStmt);
ast_node!(IfExpr, SyntaxKind::IfExpr);
ast_node!(WhileExpr, SyntaxKind::WhileExpr);
ast_node!(LoopExpr, SyntaxKind::LoopExpr);
ast_node!(ForExpr, SyntaxKind::ForExpr);
ast_node!(MatchExpr, SyntaxKind::MatchExpr);
ast_node!(MatchArmList, SyntaxKind::MatchArmList);
ast_node!(MatchArm, SyntaxKind::MatchArm);
ast_node!(MethodCallExpr, SyntaxKind::MethodCallExpr);
ast_node!(FieldExpr, SyntaxKind::FieldExpr);
ast_node!(IndexExpr, SyntaxKind::IndexExpr);
ast_node!(UnaryExpr, SyntaxKind::UnaryExpr);
ast_node!(CastExpr, SyntaxKind::CastExpr);
ast_node!(RefExpr, SyntaxKind::RefExpr);
ast_node!(ClosureExpr, SyntaxKind::ClosureExpr);
ast_node!(ArrayExpr, SyntaxKind::ArrayExpr);
ast_node!(TupleExpr, SyntaxKind::TupleExpr);
ast_node!(StructExpr, SyntaxKind::StructExpr);
ast_node!(RangeExpr, SyntaxKind::RangeExpr);
ast_node!(BreakExpr, SyntaxKind::BreakExpr);
ast_node!(ContinueExpr, SyntaxKind::ContinueExpr);
ast_node!(ReturnExpr, SyntaxKind::ReturnExpr);
ast_node!(AssignExpr, SyntaxKind::AssignExpr);
ast_node!(PathType, SyntaxKind::PathType);
ast_node!(FnType, SyntaxKind::FnType);
ast_node!(DynType, SyntaxKind::DynType);
ast_node!(RefType, SyntaxKind::RefType);
ast_node!(SliceType, SyntaxKind::SliceType);
ast_node!(ArrayType, SyntaxKind::ArrayType);
ast_node!(TupleType, SyntaxKind::TupleType);
ast_node!(NeverType, SyntaxKind::NeverType);
ast_node!(InferType, SyntaxKind::InferType);
ast_node!(GenericArgList, SyntaxKind::GenericArgList);
ast_node!(PatIdent, SyntaxKind::PatIdent);
ast_node!(PatStruct, SyntaxKind::PatStruct);
ast_node!(PatTuple, SyntaxKind::PatTuple);
ast_node!(PatOr, SyntaxKind::PatOr);
ast_node!(PatLit, SyntaxKind::PatLit);
ast_node!(PatRange, SyntaxKind::PatRange);
ast_node!(PatWild, SyntaxKind::PatWild);
ast_node!(UsePath, SyntaxKind::UsePath);
ast_node!(UseTree, SyntaxKind::UseTree);
ast_node!(MacroCall, SyntaxKind::MacroCall);
ast_node!(TokenTree, SyntaxKind::TokenTree);
ast_node!(MacroDef, SyntaxKind::MacroDef);
ast_node!(MacroArm, SyntaxKind::MacroArm);
ast_node!(MacroPattern, SyntaxKind::MacroPattern);
ast_node!(StructField, SyntaxKind::StructField);
ast_node!(EnumVariant, SyntaxKind::EnumVariant);
ast_node!(FieldList, SyntaxKind::FieldList);
ast_node!(VariantList, SyntaxKind::VariantList);

#[cfg(test)]
mod tests;
