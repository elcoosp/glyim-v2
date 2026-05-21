#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, num_enum::TryFromPrimitive, PartialOrd)]
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
    PatSlice,
}
impl SyntaxKind {
    /// Returns true for whitespace and comment tokens.
    pub fn is_trivia(&self) -> bool {
        matches!(
            self,
            SyntaxKind::Whitespace
                | SyntaxKind::LineComment
                | SyntaxKind::BlockComment
                | SyntaxKind::DocComment
        )
    }

    /// Returns true for literal tokens (integers, floats, strings, chars, booleans).
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            SyntaxKind::IntLit
                | SyntaxKind::FloatLit
                | SyntaxKind::StringLit
                | SyntaxKind::CharLit
                | SyntaxKind::BoolLit
        )
    }
}

/// Language type for rowan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlyimLang;

impl rowan::Language for GlyimLang {
    type Kind = SyntaxKind;
    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        SyntaxKind::try_from(raw.0).unwrap_or(SyntaxKind::Error)
    }
    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind as u16)
    }
}

/// Alias for rowan::SyntaxNode with GlyimLang.
pub type SyntaxNode = rowan::SyntaxNode<GlyimLang>;
/// Alias for rowan::SyntaxToken with GlyimLang.
pub type SyntaxToken = rowan::SyntaxToken<GlyimLang>;
/// Alias for rowan::SyntaxElement with GlyimLang.
pub type SyntaxElement = rowan::SyntaxElement<GlyimLang>;

pub use rowan::GreenNode;
