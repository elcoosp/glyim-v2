//! Mid-Level IR (CFG form)
use glyim_core::arena::IndexVec;
use glyim_core::def_id::DefId;
use glyim_type::Ty;
use glyim_span::Span;

glyim_core::define_idx!(BasicBlockIdx);
glyim_core::define_idx!(LocalIdx);
glyim_core::define_idx!(FieldIdx);

#[derive(Clone, Debug)]
pub struct Body {
    pub owner: DefId,
    pub basic_blocks: IndexVec<BasicBlockIdx, BasicBlockData>,
    pub locals: IndexVec<LocalIdx, LocalDecl>,
    pub arg_count: usize,
    pub return_ty: Ty,
    pub span: Span,
    pub var_debug_info: Vec<VarDebugInfo>,
}

#[derive(Clone, Debug)]
pub struct VarDebugInfo {
    pub name: glyim_core::interner::Name,
    pub value: VarDebugInfoValue,
}

#[derive(Clone, Debug)]
pub enum VarDebugInfoValue {
    Place(Place),
    Const(MirConst),
}

#[derive(Clone, Debug)]
pub struct BasicBlockData {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
    pub is_cleanup: bool,
}

impl BasicBlockData {
    pub fn new(terminator: Terminator) -> Self {
        Self { statements: Vec::new(), terminator, is_cleanup: false }
    }
}

#[derive(Clone, Debug)]
pub struct Statement {
    pub kind: StatementKind,
    pub source_info: SourceInfo,
}

#[derive(Clone, Debug)]
pub enum StatementKind {
    Assign(Place, Rvalue),
    StorageLive(LocalIdx),
    StorageDead(LocalIdx),
    Nop,
}

#[derive(Clone, Debug)]
pub enum Rvalue {
    Use(Operand),
    Ref(Place, BorrowKind),
    BinaryOp(BinOp, Box<(Operand, Operand)>),
    UnaryOp(UnOp, Operand),
    Aggregate(AggregateKind, Vec<Operand>),
    Cast(CastKind, Operand, Ty),
    Repeat(Operand, MirConst),
}

#[derive(Clone, Debug)]
pub enum AggregateKind {
    Array(Ty),
    Tuple,
    Adt(glyim_core::def_id::AdtId, VariantIdx, glyim_type::Substitution),
    Closure(glyim_core::def_id::ClosureId, glyim_type::Substitution),
}
glyim_core::define_idx!(VariantIdx);

#[derive(Clone, Debug)]
pub enum Operand {
    Copy(Place),
    Move(Place),
    Constant(MirConst),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Place {
    pub local: LocalIdx,
    pub projection: Box<[ProjectionElem]>,
}

impl Place {
    pub fn new(local: LocalIdx) -> Self {
        Self { local, projection: Box::new([]) }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectionElem {
    Deref,
    Field(FieldIdx),
    Index(LocalIdx),
    Downcast(VariantIdx),
}

#[derive(Clone, Debug)]
pub struct LocalDecl {
    pub ty: Ty,
    pub mutability: Mutability,
    pub source_info: SourceInfo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mutability { Not, Mut }

#[derive(Clone, Debug)]
pub struct MirConst {
    pub kind: MirConstKind,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum MirConstKind {
    Int(i128),
    Bool(bool),
    Unit,
    Error,
}

#[derive(Clone, Debug)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub source_info: SourceInfo,
}

#[derive(Clone, Debug)]
pub enum TerminatorKind {
    Goto { target: BasicBlockIdx },
    Return,
    Unreachable,
    Call {
        func: Operand,
        args: Vec<Operand>,
        destination: Place,
        target: Option<BasicBlockIdx>,
        cleanup: Option<BasicBlockIdx>,
    },
    Drop { place: Place, target: BasicBlockIdx, cleanup: Option<BasicBlockIdx> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BorrowKind { Shared, Unique, Mut }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastKind { IntToInt, PtrToPtr }

#[derive(Clone, Debug)]
pub struct SourceInfo { pub span: Span }
impl SourceInfo { pub fn new(span: Span) -> Self { Self { span } } }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp { Add, Sub, Eq, Ne }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp { Not, Neg }

impl Body {
    pub fn dummy(owner: DefId) -> Self {
        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(BasicBlockData::new(Terminator {
            kind: TerminatorKind::Unreachable,
            source_info: SourceInfo::new(Span::DUMMY),
        }));
        let locals = IndexVec::new();
        Self {
            owner,
            basic_blocks,
            locals,
            arg_count: 0,
            return_ty: Ty::ERROR,
            span: Span::DUMMY,
            var_debug_info: Vec::new(),
        }
    }
}
