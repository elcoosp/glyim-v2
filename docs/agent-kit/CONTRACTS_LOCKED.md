# Locked Public Contracts — v0.1.0

Generated from: actual codebase scan
Any change to items listed here requires a formal Change Request.

## glyim-core
- `pub struct Idx<T>` — `from_raw`, `to_raw`, `index`
- `pub trait IdxLike` — `from_raw`, `to_raw`, `index`
- `pub macro define_idx`
- `pub struct IndexVec<I, T>` — `new`, `with_capacity`, `from_raw`, `push`, `len`, `is_empty`, `get`, `get_mut`, `iter`, `iter_enumerated`, `into_iter_enumerated`, `into_raw`, `as_slice`, `last`
- `pub struct Name` — `as_symbol`
- `pub struct Interner` — `new`, `intern`, `resolve`, `lookup`, `clone`, `default`
- `pub struct PathKind` — `Plain`, `SelfPath`, `Super(u32)`, `Crate`
- `pub struct PathSegment` — `name: Name`
- `pub struct Path` — `from_single`, `as_name`, `segments: Vec<PathSegment>`, `kind: PathKind`
- `pub struct DefId` — `new`, `krate: CrateId`, `local_id: LocalDefId`
- `pub struct CrateId` — `from_raw`, `to_raw`
- `pub struct LocalDefId` — `from_raw`, `to_raw`
- `pub struct AdtId`, `FnDefId`, `ClosureId`, `TraitDefId`, `ImplDefId`, `OpaqueTyId`, `TypeAliasId`, `ConstDefId`, `StaticDefId` — all `from_raw`, `to_raw`
- `pub struct TargetInfo` — `x86_64()`, `pointer_width`, `pointer_size`, `pointer_align`, `default`
- `pub enum IntTy` — `I8`, `I16`, `I32`, `I64`, `Isize`; `bit_width`, `name`
- `pub enum UintTy` — `U8`, `U16`, `U32`, `U64`, `Usize`; `bit_width`, `name`
- `pub enum FloatTy` — `F32`, `F64`; `bit_width`, `name`
- `pub enum Mutability` — `Not`, `Mut`; `is_mut`, `prefix_str`
- `pub enum Safety` — `Safe`, `Unsafe`
- `pub enum Abi` — `C`, `Glyim`, `System`; `name`
- `pub enum BinOp` — all variants; `is_comparison`
- `pub enum UnOp` — `Not`, `Neg`, `Deref`
- `pub enum Visibility` — `Public`, `Module(u32)`, `Inherited`
- `pub enum StructKind` — `Unit`, `Tuple`, `Record`
- `pub fn validate_alignment`
- `pub const ALIGN_MAX`, `ALIGN_MIN`, `DEFAULT_STACK_SIZE`

## glyim-span
- `pub struct FileId` — `BOGUS`, `from_raw`, `to_raw`, `index`
- `pub struct ByteIdx` — `ZERO`, `from_raw`, `to_raw`, `to_usize`
- `pub struct Span` — `DUMMY`, `new`, `is_dummy`, `range`, `sans_ctx`, `len`, `is_empty`, `to`; fields `file`, `lo`, `hi`, `ctx`
- `pub struct SyntaxContext` — `ROOT`, `is_root`, `to_raw`
- `pub struct ExpnId` — `ROOT`, `is_root`, `to_raw`
- `pub struct HygieneKey` — (no pub constructors)
- `pub struct MultiSpan` — `from_span`, `with_secondary`; fields `primary`, `secondary`
- `pub enum Transparency` — `Transparent`, `SemiTransparent`, `Opaque`
- `pub struct Mark` — `expn_id: ExpnId`, `transparency: Transparency`
- `pub struct ExpnData` — `expn_id`, `parent`, `kind`, `call_site`, `def_site`, `transparency`
- `pub enum ExpnKind` — `MacroRules`, `ProcMacro`, `Builtin`, `Root`
- `pub struct HygieneCtx` — `new`, `push_expansion`, `apply_mark`, `remove_mark`, `expn_data`, `adjust`

## glyim-diag
- `pub struct GlyimDiagnostic` — `new`, `lex_error`, `parse_error`, `type_error`, `borrow_error`, `internal_error`, `with_source_code`, `with_sub`, `with_suggestion`, `is_error`; fields `code`, `severity`, `message`, `span`, `sub_diagnostics`, `suggestions`, `source_code`
- `pub struct DiagSink` — `new`, `with_error_limit`, `emit`, `has_errors`, `into_diagnostics`
- `pub type CompResult<T> = Result<T, Vec<GlyimDiagnostic>>`
- `pub struct ErrorCode` — `category: ErrorCategory`, `number: u16`
- `pub enum ErrorCategory` — `Lex`, `Parse`, `NameResolution`, `Type`, `Lifetime`, `Borrow`, `Comptime`, `Io`, `Internal`
- `pub enum DiagSeverity` — `Error`, `Warning`, `Note`, `Help`
- `pub struct SubDiagnostic` — `severity`, `message`, `span`
- `pub struct Suggestion` — `message`, `replacements`, `applicability`
- `pub enum Applicability` — `MachineApplicable`, `MaybeIncorrect`, `HasPlaceholders`, `Unspecified`

## glyim-syntax
- `pub enum SyntaxKind` — all 100+ variants; `is_trivia`, `is_keyword`, `is_literal`, `is_node`, `try_from_raw`
- `pub enum GlyimLang` — Rowan language impl (`kind_from_raw`, `kind_to_raw`)
- `pub type SyntaxNode`, `SyntaxToken`, `SyntaxElement`, `GreenNode`, `GreenToken`
- `pub trait AstNode` — `can_cast`, `cast`, `syntax`
- `pub fn child_of_kind`
- `pub use BinOp, UnOp` (re-exported from glyim_core)
- AST node types: `SourceFile`, `FnDef`, `StructDef`, `EnumDef`, `TraitDef`, `ImplDef`, `Block`, `CallExpr`, `BinaryExpr`, `PathExpr`, `LitExpr`

## glyim-type
- `pub struct Ty` — `to_raw`, `index`; sentinels `ERROR`, `NEVER`, `UNIT`, `BOOL`
- `pub enum TyKind` — `Bool`, `Never`, `Unit`, `Int(IntTy)`, `Uint(UintTy)`, `Float(FloatTy)`, `Char`, `String`, `Infer(InferVar)`, `Adt(AdtId, Substitution)`, `FnDef(FnDefId, Substitution)`, `Closure(ClosureId, Substitution)`, `FnPtr(FnSig)`, `Ref(Region, Ty, Mutability)`, `RawPtr(Ty, Mutability)`, `Slice(Ty)`, `Array(Ty, Const)`, `Tuple(Substitution)`, `Dynamic(Binder<Box<[Predicate]>>, Region)`, `Opaque(OpaqueTyId, Substitution)`, `Param(ParamTy)`, `Bound(u32, BoundTy)`, `Error`
- `pub enum InferVar` — `Ty(TyVar)`, `Int(IntVar)`, `Float(FloatVar)`
- `pub struct TyVar`, `IntVar`, `FloatVar` — `from_raw`, `to_raw`
- `pub struct RegionVid`, `ConstVar`, `FieldIdx` — `from_raw`, `to_raw`
- `pub struct UniverseIndex(pub u32)`
- `pub struct Substitution` — `index`, `len`, `is_empty`
- `pub enum GenericArg` — `Ty(Ty)`, `Lifetime(Region)`, `Const(Const)`
- `pub enum Region` — `Static`, `EarlyBound`, `LateBound`, `Var(RegionVid)`, `Erased`, `Error`
- `pub struct FnSig` — `inputs: Substitution`, `output: Ty`, `c_variadic: bool`, `unsafety: Safety`, `abi: Abi`
- `pub enum Predicate` — `Trait(TraitPredicate)`, `RegionOutlives`, `TypeOutlives`, `WellFormed(Ty)`, `Coerce(Ty, Ty)`
- `pub struct TraitPredicate` — `trait_ref: TraitRef`, `polarity: ImplPolarity`
- `pub struct TraitRef` — `def_id: TraitDefId`, `substs: Substitution`
- `pub struct Binder<T>` — `bind`, `skip_binder`, `as_ref`; field `bound_vars`
- `pub struct TypeFlags` — all flag constants (`HAS_TY_INFER`, `HAS_ERROR`, `HAS_DEPTH_OVERFLOW`, etc.)
- `pub fn compute_flags(kind: &TyKind, ctx: &dyn TypeLookup, depth: u32) -> TypeFlags`
- `pub trait TypeLookup` — `ty_kind`, `ty_flags`, `substitution_args`, `name_str`, `error_ty`
- `pub struct PrintTy<'a, L>` — `new(ty, lookup)`
- `pub struct TyCtxMut` — `new(Interner)`, `alloc_ty`, `mk_ty`, `mk_ref`, `freeze`, `ty_kind`, `ty_kind_mut`, `ty_flags`, `substitution_args`, `intern_substitution`, `error_ty`, `never_ty`, `unit_ty`, `bool_ty`, `resolver`, `name_str`
- `pub struct TyCtx` — `ty_kind`, `ty_flags`, `substitution_args`, `ty_is_error`, `ty_has_depth_overflow`, `error_ty`, `never_ty`, `unit_ty`, `bool_ty`, `resolver`, `name_str`, `region`
- `pub struct ParamTy` — `index`, `name`
- `pub struct BoundTy` — `var`, `kind`
- `pub enum BoundTyKind` — `Anon`, `Param(Name)`
- `pub struct Const` — `kind: ConstKind`, `ty: Ty`
- `pub enum ConstKind` — `Int`, `Uint`, `FloatBits`, `Bool`, `Char`, `String`, `Unit`, `Infer`, `Param`, `Error`
- `pub struct ParamConst` — `index`, `name`
- `pub struct EarlyBoundRegion` — `index`, `name`
- `pub struct DebruijnIndex(pub u32)` — `INNERMOST`, `shifted_in`, `shifted_out`
- `pub enum BoundRegionKind` — `BrAnon`, `BrNamed`, `BrEnv`
- `pub enum BoundVariableKind` — `Ty`, `Region`, `Const`
- `pub enum ImplPolarity` — `Positive`, `Negative`
- `pub struct RegionOutlivesPredicate` — `a`, `b`
- `pub struct TypeOutlivesPredicate` — `ty`, `region`

## glyim-mir
- `pub struct Body` — `dummy(DefId)`, `owner`, `basic_blocks`, `locals`, `arg_count`, `return_ty`, `span`, `var_debug_info`, `args()`, `return_place()`
- `pub struct Place` — `new(LocalIdx)`, `local`, `projection`; `ty(&self, ctx: &impl TypeLookup, local_decls: &IndexVec<LocalIdx, LocalDecl>) -> Ty`
- `pub enum ProjectionElem` — `Deref`, `Field(FieldIdx)`, `Index(LocalIdx)`, `Downcast(VariantIdx)`
- `pub struct LocalDecl` — `ty: Ty`, `mutability: Mutability`, `source_info: SourceInfo`
- `pub enum StatementKind` — `Assign(Place, Rvalue)`, `StorageLive(LocalIdx)`, `StorageDead(LocalIdx)`, `Nop`
- `pub enum Rvalue` — `Use(Operand)`, `Ref(Place, BorrowKind)`, `BinaryOp(BinOp, Box<(Operand, Operand)>)`, `UnaryOp(UnOp, Operand)`, `Aggregate(AggregateKind, Vec<Operand>)`, `Discriminant(Place)`, `Len(Place)`, `Cast(CastKind, Operand, Ty)`, `Repeat(Operand, MirConst)`
- `pub enum AggregateKind` — `Array(Ty)`, `Tuple`, `Adt(AdtId, VariantIdx, Substitution)`, `Closure(ClosureId, Substitution)`
- `pub enum Operand` — `Copy(Place)`, `Move(Place)`, `Constant(MirConst)`
- `pub enum TerminatorKind` — `Goto{target}`, `SwitchInt{discr, switch_ty, targets}`, `Return`, `Unreachable`, `Call{func, args, destination, target, cleanup}`, `Assert{cond, expected, target, cleanup, msg}`, `Drop{place, target, cleanup}`
- `pub enum BorrowKind` — `Shared`, `Unique`, `Mut { allow_two_phase_borrow: bool }`
- `pub enum CastKind` — `IntToInt`, `FloatToInt`, `IntToFloat`, `PtrToPtr`, `FnPtrToPtr`
- `pub struct BasicBlockData` — `new(Terminator)`; fields `statements`, `terminator`, `is_cleanup`
- `pub struct SwitchTargets` — `new`, `otherwise`, `iter`, `if_switch`
- `pub struct SourceInfo` — `new(Span)`; field `span`
- `pub struct MirConst` — `kind: MirConstKind`, `ty: Ty`, `span: Span`
- `pub enum MirConstKind` — `Int`, `Uint`, `FloatBits`, `Bool`, `Char`, `String`, `Unit`, `Error`
- `pub struct Statement` — `kind`, `source_info`
- `pub struct Terminator` — `kind`, `source_info`
- `pub enum AssertMessage` — `Overflow(BinOp)`, `DivisionByZero`, `RemainderByZero`, `BoundsCheck`
- `pub struct VarDebugInfo` — `name`, `value`
- `pub enum VarDebugInfoValue` — `Place(Place)`, `Const(MirConst)`
- `pub struct BasicBlockIdx`, `LocalIdx`, `VariantIdx` — `from_raw`, `to_raw`

## glyim-codegen
- `pub trait CodegenBackend` — `name(&self) -> &'static str`, `generate(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<Vec<u8>>`, `generate_function(&self, body: &Arc<Body>) -> CompResult<Vec<u8>>`
- `pub struct BytecodeBackend` — `new()`

## glyim-codegen-llvm
- `pub struct LlvmBackend` — `new()`, `with_target(triple: impl Into<String>)`

## glyim-db
- `pub struct Database` — `new(CrateConfig)`, `interner()`, `vfs()`, `trait_ctx()`, `krate()`, `set_ty_ctx(TyCtx)`, `ty_ctx()`
- `pub struct CrateConfig` — `name: String`, `target_triple: String`, `opt_level: u8`

## glyim-pipeline
- `pub struct Pipeline` — `compile_file(db: &mut Database, path: &Path, backend: &dyn CodegenBackend) -> CompResult<()>`

## glyim-frontend
- `pub fn lex(source: &str, file_id: FileId) -> LexResult`
- `pub fn parse_to_syntax(source: &str, file_id: FileId) -> ParseResult`
- `pub struct Token` — `kind: SyntaxKind`, `span: Span`, `text: SmolStr`, `new(kind, span, text)`
- `pub struct LexResult` — `tokens: Vec<Token>`, `diagnostics: Vec<GlyimDiagnostic>`
- `pub struct ParseResult` — `green_node: GreenNode`, `diagnostics: Vec<GlyimDiagnostic>`, `root: SyntaxNode`
- `pub struct Lexer<'a>` — `new(source, file_id)`, `lex(self) -> LexResult`

## glyim-def-map
- `pub struct CrateDefMap` — `root: ModuleId`, `modules: IndexVec<ModuleId, ModuleData>`, `krate: CrateId`
- `pub struct ModuleData` — `parent`, `children`, `scope: ItemScope`, `origin: ModuleOrigin`, `span: Span`, `resolve(Name) -> Option<(LocalDefId, Visibility)>`
- `pub struct ItemScope` — `types`, `values`, `macros`, `resolve(Name)`, `declare(name, id, vis, span, ns)`
- `pub struct PerNs` — `types`, `values`, `macros`, `is_none()`, `from_types(id, vis)`
- `pub enum Namespace` — `Types`, `Values`, `Macros`
- `pub enum ModuleOrigin` — `File{file_id}`, `Inline{span}`, `CrateRoot`
- `pub struct Resolver<'a>` — `new(def_map, module)`, `resolve_path(&Path) -> PerNs`, `def_map()`, `module()`
- `pub struct ModuleId` — `from_raw`, `to_raw`
- `pub fn build_def_map(root: &SyntaxNode, krate: CrateId) -> (CrateDefMap, Vec<GlyimDiagnostic>)`

## glyim-hir
- `pub struct CrateHir` — `items: IndexVec<ItemId, Item>`, `bodies: IndexVec<BodyId, Body>`, `body_owners: IndexVec<BodyId, LocalDefId>`
- `pub struct Item` — `id`, `name`, `kind: ItemKind`, `visibility`, `span`
- `pub enum ItemKind` — `Fn(FnItem)`, `Struct(StructItem)`, `Enum(EnumItem)`, `Trait(TraitItem)`, `Impl(ImplItem)`, `TypeAlias(TypeAliasItem)`, `Const(ConstItem)`, `Static(StaticItem)`, `Mod(ModItem)`, `Use(UseItem)`, `Extern(ExternBlockItem)`
- All HIR types: `FnItem`, `StructItem`, `EnumItem`, `TraitItem`, `ImplItem`, `TypeAliasItem`, `ConstItem`, `StaticItem`, `ModItem`, `UseItem`, `ExternBlockItem`, `Param`, `Field`, `GenericParam`, `GenericParamKind`, `Variant`, `Body`, `Expr`, `Pat`, `TypeRef`, `ConstRef`, `Literal`, `MatchArm`, `Path`, `PathSegment`, `HirId`
- `pub struct ExprId`, `PatId`, `BodyId`, `ItemId` — `from_raw`, `to_raw`

## glyim-solve
- `pub struct InferenceTable` — `new()`, `new_ty_var(&mut TyCtxMut)`, `new_int_var(&mut TyCtxMut)`, `new_float_var(&mut TyCtxMut)`, `new_region_var(&mut TyCtxMut)`, `unify(&mut TyCtxMut, Ty, Ty, Span)`, `resolve_ty_shallow(&self, &dyn TypeLookup, Ty)`, `fully_resolve(&self, &dyn TypeLookup, Ty)`, `probe_ty_var`, `probe_int_var`, `probe_float_var`, `universe()`, `create_universe()`
- `pub trait TraitSolver` — `can_prove(&mut self, &TyCtx, &TraitPredicate) -> SolverResult`, `evaluate_predicate(&mut self, &TyCtx, &Predicate) -> SolverResult`
- `pub enum SolverResult` — `Proven`, `Ambiguous`, `DefiniteNo`
- `pub struct SimpleTraitSolver<'a>` — `new(&'a TraitContext)`
- `pub struct TraitContext` — `new()`, `register_trait(TraitDef)`, `register_impl(ImplDef)`, `impls_of_trait(TraitDefId)`
- `pub struct TraitDef` — `def_id`, `name`, `associated_types`, `predicates`
- `pub struct ImplDef` — `def_id`, `trait_ref`, `predicates`
- `pub struct FulfillmentCtx<'a>` — `new(&'a TyCtx, &'a mut dyn TraitSolver)`, `register_obligation(Obligation)`, `process_obligations(usize)`, `into_diagnostics()`
- `pub struct Obligation` — `predicate: Predicate`, `cause: ObligationCause`
- `pub struct ObligationCause` — `span: Span`, `code: ObligationCauseCode`
- `pub enum ObligationCauseCode` — `WellFormed`, `TypeConstruction`, `MatchArm`, `IfThenElse`
- `pub struct OverflowError` — `predicate`, `depth`
- `pub enum Constraint` — `TypeEq`, `RegionEq`, `RegionOutlives`, `TypeOutlives`
- `pub enum VariableKind` — `General`, `Integer`, `Float`

## glyim-typeck
- `pub fn typeck_crate(ctx: TyCtxMut, def_map: &CrateDefMap, hir: &CrateHir, solver: &mut dyn TraitSolver) -> (TyCtx, TypeckResult)`
- `pub struct TypeckResult` — `expr_types`, `pat_types`, `adjustments`, `thir_bodies`, `diagnostics`
- `pub struct Adjustment` — `kind: AdjustKind`, `target: Ty`
- `pub enum AdjustKind` — `Deref`, `Borrow(Mutability)`, `NeverToAny`
- `pub struct TypeckCtx<'a>` — `ctx`, `infer`, `diagnostics`, `pending_obligations`, `unify()`, `require_trait_bound()`
- `pub mod thir` — `Body`, `Param`, `Stmt`, `Expr`, `ExprKind`, `Pattern`, `PatternKind`, `Literal`, `MatchArm`, `FieldPat`, `Capture`, `CaptureKind`, `LocalVarId`

## glyim-lower
- `pub trait LowerCtx` — `ty_ctx(&self) -> &TyCtx`, `adt_def(&self, AdtId) -> AdtDef`, `push_span(&self, Span)`, `pop_span(&self)`
- `pub fn lower_body(ctx: &dyn LowerCtx, thir: &thir::Body) -> LowerResult`
- `pub struct LowerResult` — `body: Body`, `diagnostics: Vec<GlyimDiagnostic>`
- `pub struct AdtDef` — `variants: Vec<AdtVariant>`, `kind: AdtKind`
- `pub struct AdtVariant` — `fields: Vec<Ty>`
- `pub enum AdtKind` — `Struct`, `Enum`, `Union`
- `pub struct MonoCtx` — `new()`, `collect()`, `instantiate()`, `items()`, `item_count()`
- `pub enum MonoItem` — `Fn{def_id, substs}`, `Const{def_id, substs}`, `Static{def_id}`
- `pub struct MonoItemId` — `from_raw`, `to_raw`
- `pub struct MonoItemData` — `item`, `body`, `symbol`

## glyim-borrowck
- `pub trait BorrowckCtx` — `ty_ctx(&self) -> &TyCtx`, `local_decl(&self, LocalIdx) -> &LocalDecl`, `is_copy(&self, Ty) -> bool`
- `pub fn check_borrows(ctx: &dyn BorrowckCtx, body: &Body) -> BorrowckResult`
- `pub struct BorrowckResult` — `errors: Vec<GlyimDiagnostic>`

## glyim-opt
- `pub fn optimize(ctx: &TyCtx, body: &Arc<Body>) -> Optimized`
- `pub struct Optimized` — `body: Body`

## glyim-layout
- `pub trait LayoutComputer` — `layout_of(Ty)`, `fn_abi_of(&FnSig)`, `ptr_size()`, `ptr_align()`, `target_info()`
- `pub struct SimpleLayoutComputer<'a>` — `new(&'a TyCtx, TargetInfo)`
- `pub struct Layout` — `size: Size`, `align: Align`, `fields: FieldsShape`, `variants: VariantsShape`, `is_unsized: bool`, `scalar()`, `unit()`
- `pub struct Size(pub u64)` — `ZERO`, `bytes()`, `bits()`, `align_to()`
- `pub struct Align(pub u64)` — `ONE`, `EIGHT`, `from_bytes()`, `max()`
- `pub enum LayoutError` — `UnknownType(Ty)`, `SizeOverflow(Ty)`, `Unsized(Ty)`, `Cycle(Ty)`, `AlignmentExceedsRuntime{ty, align, max}`
- `pub struct FnAbi` — `args`, `ret`, `conv`, `c_variadic`
- `pub struct ArgAbi` — `ty`, `layout`, `mode`
- `pub enum PassMode` — `Direct`, `Indirect{meta_attrs}`, `Ignore`
- `pub enum CallConvention` — `Glyim`, `C`, `System`
- `pub enum FieldsShape` — `Primitive`, `Array{stride, count}`, `Arbitrary{offsets}`
- `pub enum VariantsShape` — `Single{index}`, `Multiple{tag, tag_field, tag_encoding, variants}`
- `pub enum TagEncoding` — `Direct`, `Niche{untagged_variant, niche_variants, niche_start}`

## glyim-vfs
- `pub struct Vfs` — `new()`, `add_file_from_disk(&Path) -> io::Result<FileId>`, `add_file_content(&Path, Arc<str>) -> FileId`, `set_file_content(FileId, Arc<str>)`, `file_content(FileId) -> Option<Arc<str>>`, `file_content_ref(FileId, f)`, `file_path(FileId)`, `file_id(&Path)`, `len()`, `is_empty()`

## glyim-meta
- `pub enum MacroKind` — `Declarative{name}`, `Proc{name}`, `Builtin{name, handler}`
- `pub enum BuiltinMacro` — `File`, `Line`, `Column`, `Include`, `Env`
- `pub struct MacroDef` — `name`, `kind`, `span`
- `pub struct ExpansionResult` — `expanded`, `diagnostics`
- `pub struct Expander<'a>` — `new(&'a mut HygieneCtx)`, `register_macro(MacroDef)`, `expand(Name, &SyntaxNode, Span)`, `expand_crate(&SyntaxNode)`

## glyim-runtime
- `pub fn glyim_alloc`, `glyim_dealloc`, `glyim_panic`
- `pub use ALIGN_MAX`

## glyim-cli
- `pub struct CliArgs` — `input`, `output`, `opt_level`, `target`
- `pub fn run() -> Result<(), Vec<GlyimDiagnostic>>`

## glyim-lsp
- (Empty stub — no public items yet)

## glyim-mir-interp
- (Empty stub — no public items yet)
