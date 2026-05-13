# Locked Public Contracts — v0.1.0

Generated from: `contracts-locked-v0.1.0` tag
Any change to items listed here requires a formal Change Request.

## glyim-core
- `pub struct Idx<T>` — `from_raw`, `to_raw`, `index`
- `pub trait IdxLike` — `from_raw`, `to_raw`, `index`
- `pub macro define_idx`
- `pub struct IndexVec<I, T>` — all methods
- `pub struct Name` — `as_symbol`
- `pub struct Interner` — `new`, `intern`, `resolve`, `lookup`
- `pub struct PathKind` — `Plain`, `SelfPath`, `Super(u32)`, `Crate`
- `pub struct PathSegment` — `name`
- `pub struct Path` — `from_single`, `as_name`, `segments`, `kind`
- `pub struct DefId` — `new`, `krate`, `local_id`
- `pub struct CrateId` — `from_raw`, `to_raw`
- `pub enum IntTy` — all variants and methods
- `pub enum UintTy` — all variants and methods
- `pub enum FloatTy` — all variants and methods
- `pub enum Mutability` — `is_mut`, `prefix_str`
- `pub enum BinOp` — `is_comparison`
- `pub enum UnOp`
- `pub enum Visibility`
- `pub enum StructKind`
- `pub fn validate_alignment`
- `pub const ALIGN_MAX`, `ALIGN_MIN`, `DEFAULT_STACK_SIZE`

## glyim-span
- `pub struct FileId` — `BOGUS`, `from_raw`, `to_raw`, `index`
- `pub struct ByteIdx` — `ZERO`, `from_raw`, `to_raw`, `to_usize`
- `pub struct Span` — `DUMMY`, `new`, `is_dummy`, `range`, `sans_ctx`, `len`, `to`
- `pub struct SyntaxContext` — `ROOT`, `is_root`, `to_raw`
- `pub struct ExpnId` — `ROOT`, `is_root`, `to_raw`
- `pub struct HygieneKey` — (no pub constructors)
- `pub struct MultiSpan` — `from_span`, `with_secondary`
- `pub struct HygieneCtx` — `new`, `push_expansion`, `apply_mark`, `remove_mark`, `expn_data`, `adjust`

## glyim-diag
- `pub struct GlyimDiagnostic` — all constructors, `with_source_code`, `with_sub`, `with_suggestion`, `is_error`
- `pub struct DiagSink` — `new`, `with_error_limit`, `emit`, `has_errors`, `into_diagnostics`
- `pub type CompResult<T>`
- `pub struct ErrorCode`, `ErrorCategory`
- `pub enum DiagSeverity`

## glyim-syntax
- `pub enum SyntaxKind` — all variants, `is_trivia`, `is_keyword`, `is_literal`, `is_node`, `try_from_raw`
- `pub enum GlyimLang` — Rowan language impl
- `pub type SyntaxNode`, `SyntaxToken`, `SyntaxElement`, `GreenNode`, `GreenToken`
- `pub trait AstNode` — `can_cast`, `cast`, `syntax`
- `pub fn child_of_kind`
- AST node types: `SourceFile`, `FnDef`, `StructDef`, `EnumDef`, `TraitDef`, `ImplDef`, `Block`, `CallExpr`, `BinaryExpr`, `PathExpr`, `LitExpr`

## glyim-type
- `pub struct Ty` — `to_raw`, `index`, `ERROR`, `NEVER`, `UNIT`, `BOOL`
- `pub enum TyKind` — all variants
- `pub enum InferVar` — `Ty(TyVar)`, `Int(IntVar)`, `Float(FloatVar)`
- `pub struct TyVar`, `IntVar`, `FloatVar`, `RegionVid`, `ConstVar`, `FieldIdx`
- `pub struct Substitution` — `index`, `len`, `is_empty`
- `pub enum GenericArg` — `Ty`, `Lifetime`, `Const`
- `pub enum Region` — all variants
- `pub struct FnSig`
- `pub enum Predicate`, `TraitPredicate`, `TraitRef`
- `pub struct Binder<T>` — `bind`, `skip_binder`, `as_ref`
- `pub struct TypeFlags` — all flag constants
- `pub fn compute_flags<L: TypeLookup>`
- `pub trait TypeLookup` — `ty_kind`, `ty_flags`, `substitution_args`, `name_str`, `error_ty`
- `pub struct PrintTy<'a, L>` — `new`
- `pub struct TyCtxMut` — `new`, `alloc_ty`, `mk_*`, `freeze`, `ty_kind`, `ty_flags`, `substitution_args`, `error_ty`
- `pub struct TyCtx` — `ty_kind`, `ty_flags`, `substitution_args`, `ty_is_error`, `ty_has_depth_overflow`, `error_ty`, `never_ty`, `unit_ty`, `bool_ty`

## glyim-mir
- `pub struct Body` — `dummy`, `owner`, `basic_blocks`, `locals`, `arg_count`, `return_ty`
- `pub struct Place` — `new`, `local`, `projection`
- `pub enum ProjectionElem` — `Deref`, `Field`, `Index`, `Downcast`
- `pub struct LocalDecl` — `ty`, `mutability`, `source_info`
- `pub enum StatementKind` — all variants
- `pub enum Rvalue` — all variants
- `pub enum TerminatorKind` — all variants
- `pub struct BasicBlockData` — `new`

## glyim-codegen
- `pub trait CodegenBackend` — `name`, `generate`
- `pub struct BytecodeBackend` — `new`

## glyim-codegen-llvm
- `pub struct LlvmBackend` — `new`

## glyim-db
- `pub struct Database` — `new`, `interner`, `vfs`

## glyim-pipeline
- `pub struct Pipeline` — `compile_file`

## glyim-frontend
- `pub fn lex(source: &str, file_id: FileId) -> LexResult`
- `pub fn parse_to_syntax(source: &str, file_id: FileId) -> ParseResult`
- `pub struct Token` — `kind`, `span`, `text`
- `pub struct LexResult` — `tokens`, `diagnostics`
- `pub struct ParseResult` — `green_node`, `diagnostics`, `root`

## glyim-def-map
- `pub struct CrateDefMap` — `root`, `modules`, `krate`
- `pub fn build_def_map(root: &SyntaxNode, krate: CrateId) -> (CrateDefMap, Vec<GlyimDiagnostic>)`

## glyim-hir
- `pub struct CrateHir` — `items`, `bodies`, `body_owners`
- All HIR types: `Item`, `ItemKind`, `FnItem`, `StructItem`, `EnumItem`, `Body`, `Expr`, `Pat`, `TypeRef`, `Path`

## glyim-solve
- `pub struct InferenceTable` — `new`
- `pub trait TraitSolver`
- `pub struct SimpleTraitSolver`
- `pub struct TraitContext`
- `pub struct FulfillmentCtx`

## glyim-typeck
- `pub fn typeck_crate(ctx: TyCtxMut, def_map: &CrateDefMap, hir: &CrateHir, solver: &mut dyn TraitSolver) -> (TyCtx, TypeckResult)`

## glyim-lower
- `pub trait LowerCtx`
- `pub fn lower_body(ctx: &dyn LowerCtx, thir: &ThirBody) -> LowerResult`

## glyim-borrowck
- `pub trait BorrowckCtx`
- `pub fn check_borrows(ctx: &dyn BorrowckCtx, body: &Body) -> BorrowckResult`

## glyim-opt
- `pub fn optimize(ctx: &TyCtx, body: &Arc<Body>) -> Optimized`

## glyim-layout
- `pub struct SimpleLayoutComputer<'a>` — `new`

## glyim-vfs
- `pub struct Vfs` — `new`, `add_file_from_disk`, `add_file_content`, `file_content`, `file_id`

## glyim-runtime
- `pub fn glyim_alloc`, `glyim_dealloc`, `glyim_panic`
- `pub use ALIGN_MAX`
