# Locked Public Contracts — v0.1.0

Generated from: actual codebase scan (2026-05-14)  
Any change to items listed here requires a formal Change Request.

---

## glyim-core

- `pub struct Idx<T>` — `from_raw`, `to_raw`, `index`
- `pub trait IdxLike` — `from_raw`, `to_raw`, `index`
- `pub macro define_idx`
- `pub struct IndexVec<I: IdxLike, T>` — `new`, `with_capacity`, `from_raw`, `push`, `len`, `is_empty`, `get`, `get_mut`, `iter`, `iter_enumerated`, `into_iter_enumerated`, `into_raw`, `as_slice`, `last`
- `pub struct Name` — `as_symbol`
- `pub struct Interner` — `new`, `intern`, `resolve`, `lookup`, `clone`, `default`
- `pub struct PathKind` — `Plain`, `SelfPath`, `Super(u32)`, `Crate`
- `pub struct PathSegment` — `name: Name`
- `pub struct Path` — `from_single`, `as_name`, `segments: Vec<PathSegment>`, `kind: PathKind`
- `pub struct DefId` — `new`, `krate: CrateId`, `local_id: LocalDefId`
- `pub struct CrateId` — `from_raw`, `to_raw`
- `pub struct LocalDefId` — `from_raw`, `to_raw`
- `pub struct AdtId`, `FnDefId`, `ClosureId`, `TraitDefId`, `ImplDefId`, `OpaqueTyId`, `TypeAliasId`, `ConstDefId`, `StaticDefId` — each with `from_raw`, `to_raw`
- `pub struct TargetInfo` — `x86_64`, `pointer_width`, `pointer_size`, `pointer_align`, `default`
- `pub enum IntTy` — `I8`, `I16`, `I32`, `I64`, `Isize`; `bit_width`, `name`
- `pub enum UintTy` — `U8`, `U16`, `U32`, `U64`, `Usize`; `bit_width`, `name`
- `pub enum FloatTy` — `F32`, `F64`; `bit_width`, `name`
- `pub enum Mutability` — `Not`, `Mut`; `is_mut`, `prefix_str`
- `pub enum Safety` — `Safe`, `Unsafe`
- `pub enum Abi` — `C`, `Glyim`, `System`; `name`
- `pub enum BinOp` — all variants (`Add`, `Sub`, `Mul`, `Div`, `Rem`, `Eq`, `Ne`, `Lt`, `Gt`, `LtEq`, `GtEq`, `And`, `Or`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`); `is_comparison`
- `pub enum UnOp` — `Not`, `Neg`, `Deref`
- `pub enum Visibility` — `Public`, `Module(u32)`, `Inherited`
- `pub enum StructKind` — `Unit`, `Tuple`, `Record`
- `pub fn validate_alignment(align: u64) -> Result<(), String>`
- `pub const ALIGN_MAX: u64`, `ALIGN_MIN: u64`, `DEFAULT_STACK_SIZE: usize`

---

## glyim-span

- `pub struct FileId` — `BOGUS`, `from_raw`, `to_raw`, `index`
- `pub struct ByteIdx` — `ZERO`, `from_raw`, `to_raw`, `to_usize`
- `pub struct Span` — `DUMMY`, `new`, `is_dummy`, `range`, `sans_ctx`, `len`, `is_empty`, `to`; fields: `file: FileId`, `lo: ByteIdx`, `hi: ByteIdx`, `ctx: SyntaxContext`
- `pub struct SyntaxContext` — `ROOT`, `is_root`, `to_raw`
- `pub struct ExpnId` — `ROOT`, `is_root`, `to_raw`
- `pub struct HygieneKey` — (no public constructors)
- `pub struct MultiSpan` — `from_span`, `with_secondary`; fields `primary: Span`, `secondary: Vec<(Span, String)>`
- `pub enum Transparency` — `Transparent`, `SemiTransparent`, `Opaque`
- `pub struct Mark` — `expn_id: ExpnId`, `transparency: Transparency`
- `pub struct ExpnData` — `expn_id`, `parent`, `kind`, `call_site`, `def_site`, `transparency`
- `pub enum ExpnKind` — `MacroRules { name: Name }`, `ProcMacro { name: Name }`, `Builtin { name: Name }`, `Root`
- `pub struct HygieneCtx` — `new`, `push_expansion`, `apply_mark`, `remove_mark`, `expn_data`, `adjust`

---

## glyim-diag

- `pub struct GlyimDiagnostic` — `new`, `lex_error`, `parse_error`, `type_error`, `borrow_error`, `internal_error`, `with_source_code`, `with_sub`, `with_suggestion`, `is_error`; fields: `code: ErrorCode`, `severity: DiagSeverity`, `message: String`, `span: MultiSpan`, `sub_diagnostics: Vec<SubDiagnostic>`, `suggestions: Vec<Suggestion>`, `source_code: Option<Arc<str>>`
- `pub struct DiagSink` — `new`, `with_error_limit`, `with_on_emit`, `emit`, `has_errors`, `into_diagnostics`
- `pub type CompResult<T> = Result<T, Vec<GlyimDiagnostic>>`
- `pub struct ErrorCode` — `category: ErrorCategory`, `number: u16`
- `pub enum ErrorCategory` — `Lex`, `Parse`, `NameResolution`, `Type`, `Lifetime`, `Borrow`, `Comptime`, `Io`, `Internal`
- `pub enum DiagSeverity` — `Error`, `Warning`, `Note`, `Help`
- `pub struct SubDiagnostic` — `severity: DiagSeverity`, `message: String`, `span: Option<MultiSpan>`
- `pub struct Suggestion` — `message: String`, `replacements: Vec<(Span, String)>`, `applicability: Applicability`
- `pub enum Applicability` — `MachineApplicable`, `MaybeIncorrect`, `HasPlaceholders`, `Unspecified`

---

## glyim-syntax

- `pub enum SyntaxKind` — all 100+ variants (keywords, literals, operators, punctuation, delimiters, trivia, nodes); `is_trivia`, `is_keyword`, `is_literal`, `is_node`, `try_from_raw`
- `pub enum GlyimLang` — implements `rowan::Language` (`kind_from_raw`, `kind_to_raw`)
- `pub type SyntaxNode = rowan::SyntaxNode<GlyimLang>`
- `pub type SyntaxToken = rowan::SyntaxToken<GlyimLang>`
- `pub type SyntaxElement = rowan::SyntaxElement<GlyimLang>`
- `pub type GreenNode = rowan::GreenNode`
- `pub type GreenToken = rowan::GreenToken`
- `pub trait AstNode` — `can_cast(kind: SyntaxKind) -> bool`, `cast(node: SyntaxNode) -> Option<Self>`, `syntax(&self) -> &SyntaxNode`
- `pub fn child_of_kind(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode>`
- AST node types: `SourceFile`, `FnDef`, `StructDef`, `EnumDef`, `TraitDef`, `ImplDef`, `Block`, `CallExpr`, `BinaryExpr`, `PathExpr`, `LitExpr` (each wraps `SyntaxNode`, implements `AstNode`)
- Re‑export `BinOp`, `UnOp` from `glyim_core`

---

## glyim-type

- `pub struct Ty` — `to_raw`, `index`; sentinels `ERROR`, `NEVER`, `UNIT`, `BOOL`
- `pub enum TyKind` — `Never`, `Unit`, `Bool`, `Int(IntTy)`, `Uint(UintTy)`, `Float(FloatTy)`, `Char`, `String`, `Infer(InferVar)`, `Adt(AdtId, Substitution)`, `FnDef(FnDefId, Substitution)`, `Closure(ClosureId, Substitution)`, `FnPtr(FnSig)`, `Ref(Region, Ty, Mutability)`, `RawPtr(Ty, Mutability)`, `Slice(Ty)`, `Array(Ty, Const)`, `Tuple(Substitution)`, `Dynamic(Binder<Box<[Predicate]>>, Region)`, `Opaque(OpaqueTyId, Substitution)`, `Param(ParamTy)`, `Bound(u32, BoundTy)`, `Error`
- `pub enum InferVar` — `Ty(TyVar)`, `Int(IntVar)`, `Float(FloatVar)`
- `pub struct TyVar`, `IntVar`, `FloatVar`, `RegionVid`, `ConstVar`, `FieldIdx` — each with `from_raw`, `to_raw`
- `pub struct UniverseIndex(pub u32)`
- `pub struct Substitution` — `index`, `len`, `is_empty`
- `pub enum GenericArg` — `Ty(Ty)`, `Lifetime(Region)`, `Const(Const)`
- `pub enum Region` — `Static`, `EarlyBound(EarlyBoundRegion)`, `LateBound(DebruijnIndex, u32, BoundRegionKind)`, `Var(RegionVid)`, `Erased`, `Error`
- `pub struct FnSig` — `inputs: Substitution`, `output: Ty`, `c_variadic: bool`, `unsafety: Safety`, `abi: Abi`
- `pub enum Predicate` — `Trait(TraitPredicate)`, `RegionOutlives(RegionOutlivesPredicate)`, `TypeOutlives(TypeOutlivesPredicate)`, `WellFormed(Ty)`, `Coerce(Ty, Ty)`
- `pub struct TraitPredicate` — `trait_ref: TraitRef`, `polarity: ImplPolarity`
- `pub struct TraitRef` — `def_id: TraitDefId`, `substs: Substitution`
- `pub struct Binder<T>` — `bind`, `skip_binder`, `as_ref`; field `bound_vars: Box<[BoundVariableKind]>`
- `pub struct TypeFlags` — constants: `HAS_TY_INFER`, `HAS_TY_PARAM`, `HAS_RE_INFER`, `HAS_RE_PARAM`, `HAS_CT_INFER`, `HAS_CT_PARAM`, `HAS_ERROR`, `HAS_DEPTH_OVERFLOW`
- `pub fn compute_flags(kind: &TyKind, ctx: &dyn TypeLookup, depth: u32) -> TypeFlags`
- `pub trait TypeLookup` — `ty_kind(&self, Ty) -> &TyKind`, `ty_flags(&self, Ty) -> TypeFlags`, `substitution_args(&self, Substitution) -> &[GenericArg]`, `name_str(&self, Name) -> &str`, `error_ty(&self) -> Ty`
- `pub struct PrintTy<'a, L: TypeLookup>` — `new(ty: Ty, lookup: &'a L)`
- `pub struct TyCtxMut` — `new(resolver: Interner)`, `alloc_ty`, `mk_ty`, `mk_ref`, `mk_adt`, `mk_tuple`, `mk_fn_ptr`, `error_ty`, `never_ty`, `unit_ty`, `bool_ty`, `freeze`, `ty_kind`, `ty_kind_mut`, `ty_flags`, `substitution_args`, `intern_substitution`, `resolver`, `name_str`, `new_region_var`, `region_var`, `region_var_count`
- `pub struct TyCtx` — `ty_kind`, `ty_flags`, `substitution_args`, `region`, `resolver`, `name_str`, `error_ty`, `never_ty`, `unit_ty`, `bool_ty`, `ty_is_error`, `ty_has_depth_overflow`
- `pub struct ParamTy` — `index: u32`, `name: Name`
- `pub struct BoundTy` — `var: u32`, `kind: BoundTyKind`
- `pub enum BoundTyKind` — `Anon`, `Param(Name)`
- `pub struct Const` — `kind: ConstKind`, `ty: Ty`
- `pub enum ConstKind` — `Int(i128)`, `Uint(u128)`, `FloatBits(u64)`, `Bool(bool)`, `Char(char)`, `String(Name)`, `Unit`, `Infer(ConstVar)`, `Param(ParamConst)`, `Error`
- `pub struct ParamConst` — `index: u32`, `name: Name`
- `pub struct EarlyBoundRegion` — `index: u32`, `name: Name`
- `pub struct DebruijnIndex(pub u32)` — `INNERMOST`, `shifted_in`, `shifted_out`
- `pub enum BoundRegionKind` — `BrAnon(u32)`, `BrNamed(Name)`, `BrEnv`
- `pub enum BoundVariableKind` — `Ty(BoundTyKind)`, `Region(BoundRegionKind)`, `Const`
- `pub enum ImplPolarity` — `Positive`, `Negative`
- `pub struct RegionOutlivesPredicate` — `a: Region`, `b: Region`
- `pub struct TypeOutlivesPredicate` — `ty: Ty`, `region: Region`

---

## glyim-mir

- `pub struct Body` — `dummy(owner: DefId)`, `owner: DefId`, `basic_blocks: IndexVec<BasicBlockIdx, BasicBlockData>`, `locals: IndexVec<LocalIdx, LocalDecl>`, `arg_count: usize`, `return_ty: Ty`, `span: Span`, `var_debug_info: Vec<VarDebugInfo>`, `args() -> &[LocalDecl]`, `return_place() -> Place`
- `pub struct Place` — `new(local: LocalIdx)`, `local: LocalIdx`, `projection: Box<[ProjectionElem]>`, `ty(&self, ctx: &impl TypeLookup, local_decls: &IndexVec<LocalIdx, LocalDecl>) -> Ty`
- `pub enum ProjectionElem` — `Deref`, `Field(FieldIdx)`, `Index(LocalIdx)`, `Downcast(VariantIdx)`
- `pub struct LocalDecl` — `ty: Ty`, `mutability: Mutability`, `source_info: SourceInfo`
- `pub enum StatementKind` — `Assign(Place, Rvalue)`, `StorageLive(LocalIdx)`, `StorageDead(LocalIdx)`, `Nop`
- `pub enum Rvalue` — `Use(Operand)`, `Ref(Place, BorrowKind)`, `BinaryOp(BinOp, Box<(Operand, Operand)>)`, `UnaryOp(UnOp, Operand)`, `Aggregate(AggregateKind, Vec<Operand>)`, `Discriminant(Place)`, `Len(Place)`, `Cast(CastKind, Operand, Ty)`, `Repeat(Operand, MirConst)`
- `pub enum AggregateKind` — `Array(Ty)`, `Tuple`, `Adt(AdtId, VariantIdx, Substitution)`, `Closure(ClosureId, Substitution)`
- `pub enum Operand` — `Copy(Place)`, `Move(Place)`, `Constant(MirConst)`
- `pub struct MirConst` — `kind: MirConstKind`, `ty: Ty`, `span: Span`
- `pub enum MirConstKind` — `Int(i128)`, `Uint(u128)`, `FloatBits(u64)`, `Bool(bool)`, `Char(char)`, `String(Name)`, `Unit`, `Error`
- `pub struct Terminator` — `kind: TerminatorKind`, `source_info: SourceInfo`
- `pub enum TerminatorKind` — `Goto { target: BasicBlockIdx }`, `SwitchInt { discr: Operand, switch_ty: Ty, targets: SwitchTargets }`, `Return`, `Unreachable`, `Call { func: Operand, args: Vec<Operand>, destination: Place, target: Option<BasicBlockIdx>, cleanup: Option<BasicBlockIdx> }`, `Assert { cond: Operand, expected: bool, target: BasicBlockIdx, cleanup: Option<BasicBlockIdx>, msg: AssertMessage }`, `Drop { place: Place, target: BasicBlockIdx, cleanup: Option<BasicBlockIdx> }`
- `pub enum BorrowKind` — `Shared`, `Unique`, `Mut { allow_two_phase_borrow: bool }`
- `pub enum CastKind` — `IntToInt`, `FloatToInt`, `IntToFloat`, `PtrToPtr`, `FnPtrToPtr`
- `pub struct BasicBlockData` — `new(terminator: Terminator)`, `statements: Vec<Statement>`, `terminator: Terminator`, `is_cleanup: bool`
- `pub struct SwitchTargets` — `new(branches: Box<[(u128, BasicBlockIdx)]>, otherwise: BasicBlockIdx)`, `otherwise()`, `iter()`, `if_switch(then_bb, else_bb)`
- `pub struct SourceInfo` — `new(span: Span)`, `span: Span`
- `pub struct Statement` — `kind: StatementKind`, `source_info: SourceInfo`
- `pub enum AssertMessage` — `Overflow(BinOp)`, `DivisionByZero`, `RemainderByZero`, `BoundsCheck`
- `pub struct VarDebugInfo` — `name: Name`, `value: VarDebugInfoValue`
- `pub enum VarDebugInfoValue` — `Place(Place)`, `Const(MirConst)`
- `pub struct BasicBlockIdx`, `LocalIdx`, `VariantIdx` — each `from_raw`, `to_raw`

---

## glyim-codegen

- `pub trait CodegenBackend` — `name(&self) -> &'static str`, `generate(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<()>`, `generate_function(&self, body: &Arc<Body>) -> CompResult<Vec<u8>>`
- `pub struct BytecodeBackend` — `new()`, `default`

---

## glyim-codegen-llvm

- `pub struct LlvmBackend` — `new()`, `with_target(target_triple: impl Into<String>)`, `default`

---

## glyim-db

- `pub struct Database` — `new(config: CrateConfig)`, `interner(&self) -> &Interner`, `vfs(&self) -> &Vfs`, `krate(&self) -> CrateId`, `set_ty_ctx(&self, ctx: TyCtx)`, `ty_ctx(&self) -> parking_lot::RwLockReadGuard<'_, Option<TyCtx>>`, `intern_mut(&mut self) -> &mut Interner`
- `pub struct CrateConfig` — `name: String`, `target_triple: String`, `opt_level: u8`

> **Change from previous contract:** Removed `trait_ctx()` (no longer present) and `db_helpers` module (functionality moved to `Database::intern_mut`).

---

## glyim-pipeline

- `pub struct Pipeline` — `compile_file(db: &mut Database, path: &Path, backend: &dyn CodegenBackend) -> CompResult<()>`

---

## glyim-frontend

- `pub fn lex(source: &str, file_id: FileId) -> LexResult`
- `pub fn parse_to_syntax(source: &str, file_id: FileId) -> ParseResult`
- `pub struct Token` — `kind: SyntaxKind`, `span: Span`, `text: SmolStr`, `new(kind, span, text)`
- `pub struct LexResult` — `tokens: Vec<Token>`, `diagnostics: Vec<GlyimDiagnostic>`
- `pub struct ParseResult` — `green_node: GreenNode`, `diagnostics: Vec<GlyimDiagnostic>`, `root: SyntaxNode`
- `pub struct Lexer<'a>` — `new(source: &'a str, file_id: FileId)`, `lex(self) -> LexResult`

---

## glyim-def-map

- `pub struct CrateDefMap` — `root: ModuleId`, `modules: IndexVec<ModuleId, ModuleData>`, `krate: CrateId`, `interner: Interner`
- `pub struct ModuleData` — `parent: Option<ModuleId>`, `children: Vec<(Name, ModuleId)>`, `scope: ItemScope`, `origin: ModuleOrigin`, `span: Span`, `resolve(&self, name: Name) -> Option<(LocalDefId, Visibility)>`
- `pub struct ItemScope` — `types: Vec<(Name, LocalDefId, Visibility, Span)>`, `values: Vec<(Name, LocalDefId, Visibility, Span)>`, `macros: Vec<(Name, LocalDefId, Visibility, Span)>`, `resolve(&self, name: Name) -> Option<(LocalDefId, Visibility)>`, `declare(&mut self, name: Name, id: LocalDefId, vis: Visibility, span: Span, ns: Namespace)`
- `pub struct PerNs` — `types: Option<(LocalDefId, Visibility)>`, `values: Option<(LocalDefId, Visibility)>`, `macros: Option<(LocalDefId, Visibility)>`, `is_none(&self) -> bool`, `from_types(id: LocalDefId, vis: Visibility) -> Self`
- `pub enum Namespace` — `Types`, `Values`, `Macros`
- `pub enum ModuleOrigin` — `File { file_id: FileId }`, `Inline { span: Span }`, `CrateRoot`
- `pub struct Resolver<'a>` — `new(def_map: &'a CrateDefMap, module: ModuleId)`, `resolve_path(&self, path: &Path) -> PerNs`, `def_map(&self) -> &CrateDefMap`, `module(&self) -> ModuleId`
- `pub struct ModuleId` — `from_raw`, `to_raw`
- `pub fn build_def_map(root: &SyntaxNode, krate: CrateId) -> (CrateDefMap, Vec<GlyimDiagnostic>)`

---

## glyim-hir

- `pub struct CrateHir` — `items: IndexVec<ItemId, Item>`, `bodies: IndexVec<BodyId, Body>`, `body_owners: IndexVec<BodyId, LocalDefId>`
- `pub struct Item` — `id: ItemId`, `name: Name`, `kind: ItemKind`, `visibility: Visibility`, `span: Span`
- `pub enum ItemKind` — `Fn(FnItem)`, `Struct(StructItem)`, `Enum(EnumItem)`, `Trait(TraitItem)`, `Impl(ImplItem)`, `TypeAlias(TypeAliasItem)`, `Const(ConstItem)`, `Static(StaticItem)`, `Mod(ModItem)`, `Use(UseItem)`, `Extern(ExternBlockItem)`
- `pub struct FnItem` — `params: Vec<Param>`, `return_ty: Option<TypeRef>`, `body: Option<BodyId>`, `is_unsafe: bool`, `is_async: bool`, `generic_params: Vec<GenericParam>`
- `pub struct StructItem` — `fields: Vec<Field>`, `kind: StructKind`, `generic_params: Vec<GenericParam>`
- `pub struct EnumItem` — `variants: Vec<Variant>`, `generic_params: Vec<GenericParam>`
- `pub struct TraitItem` — `associated_types: Vec<Name>`, `methods: Vec<Name>`, `generic_params: Vec<GenericParam>`
- `pub struct ImplItem` — `trait_ref: Option<Path>`, `self_ty: TypeRef`, `methods: Vec<Name>`, `generic_params: Vec<GenericParam>`
- `pub struct TypeAliasItem` — `ty: Option<TypeRef>`, `generic_params: Vec<GenericParam>`
- `pub struct ConstItem` — `ty: TypeRef`, `body: Option<BodyId>`
- `pub struct StaticItem` — `ty: TypeRef`, `body: Option<BodyId>`, `is_mut: bool`
- `pub struct ModItem` — `children: Vec<ItemId>`
- `pub struct UseItem` — `path: Path`, `alias: Option<Name>`
- `pub struct ExternBlockItem` — `items: Vec<ItemId>`, `abi: Option<Name>`
- `pub struct Param` — `name: Name`, `ty: Option<TypeRef>`, `span: Span`
- `pub struct Field` — `name: Name`, `ty: TypeRef`, `span: Span`
- `pub struct GenericParam` — `name: Name`, `kind: GenericParamKind`, `span: Span`
- `pub enum GenericParamKind` — `Type { default: Option<TypeRef> }`, `Lifetime`, `Const { ty: TypeRef, default: Option<ConstRef> }`
- `pub struct Variant` — `name: Name`, `fields: Vec<Field>`, `kind: StructKind`, `span: Span`
- `pub struct Body` — `owner: LocalDefId`, `exprs: IndexVec<ExprId, Expr>`, `pats: IndexVec<PatId, Pat>`, `params: Vec<PatId>`, `span: Span`
- `pub enum Expr` — `Missing`, `Path(Path)`, `Literal(Literal)`, `Block { stmts: Vec<ExprId>, tail: Option<ExprId> }`, `If { cond: ExprId, then_branch: ExprId, else_branch: Option<ExprId> }`, `While { cond: ExprId, body: ExprId }`, `Loop { body: ExprId }`, `For { pat: PatId, iterable: ExprId, body: ExprId }`, `Match { scrutinee: ExprId, arms: Vec<MatchArm> }`, `Call { func: ExprId, args: Vec<ExprId> }`, `MethodCall { receiver: ExprId, method: Name, args: Vec<ExprId> }`, `Field { receiver: ExprId, field: Name }`, `Index { base: ExprId, index: ExprId }`, `Unary { op: UnOp, expr: ExprId }`, `Binary { op: BinOp, lhs: ExprId, rhs: ExprId }`, `Cast { expr: ExprId, ty: TypeRef }`, `Ref { expr: ExprId, mutability: Mutability }`, `Assign { lhs: ExprId, rhs: ExprId }`, `Return { value: Option<ExprId> }`, `Break { value: Option<ExprId> }`, `Continue`, `Closure { params: Vec<PatId>, body: ExprId }`, `Array(Vec<ExprId>)`, `Tuple(Vec<ExprId>)`, `Struct { path: Path, fields: Vec<(Name, ExprId)>, spread: Option<ExprId> }`, `Range { start: Option<ExprId>, end: Option<ExprId>, inclusive: bool }`, `Err`
- `pub struct MatchArm` — `pat: PatId`, `guard: Option<ExprId>`, `body: ExprId`
- `pub enum Pat` — `Wild`, `Binding { name: Name, mutability: Mutability, subpattern: Option<PatId> }`, `Struct { path: Path, fields: Vec<(Name, PatId)>, rest: bool }`, `Tuple(Vec<PatId>)`, `Or(Vec<PatId>)`, `Literal(Literal)`, `Range { start: Option<Literal>, end: Option<Literal>, inclusive: bool }`, `Path(Path)`, `Err`
- `pub enum TypeRef` — `Path(Path)`, `Fn { params: Vec<TypeRef>, ret: Option<Box<TypeRef>> }`, `Ref { inner: Box<TypeRef>, mutability: Mutability }`, `Slice(Box<TypeRef>)`, `Array { inner: Box<TypeRef>, len: ConstRef }`, `Tuple(Vec<TypeRef>)`, `Never`, `Infer`, `Error`
- `pub enum ConstRef` — `Literal(Literal)`, `Path(Path)`, `Error`
- `pub enum Literal` — `Int(i128, Option<IntTy>)`, `Uint(u128, Option<UintTy>)`, `Float(u64, FloatTy)`, `Bool(bool)`, `Char(char)`, `String(Name)`, `Unit`
- `pub struct Path` — `segments: Vec<PathSegment>`, `kind: PathKind`, `from_single(name: Name) -> Self`, `as_name(&self) -> Option<Name>`
- `pub struct PathSegment` — `name: Name`, `generic_args: Option<Vec<TypeRef>>`
- `pub struct HirId` — `owner: LocalDefId`, `local: u32`
- `pub struct ExprId`, `PatId`, `BodyId`, `ItemId` — `from_raw`, `to_raw`
- `pub mod pipeline_api` — `pub fn lower_crate_for_pipeline(root: &SyntaxNode, interner: &mut Interner) -> CrateHir`

---

## glyim-solve

- `pub struct InferenceTable` — `new()`, `new_ty_var(&mut TyCtxMut) -> TyVar`, `new_int_var(&mut TyCtxMut) -> IntVar`, `new_float_var(&mut TyCtxMut) -> FloatVar`, `new_region_var(&mut TyCtxMut) -> RegionVid`, `unify(&mut self, ctx: &mut TyCtxMut, a: Ty, b: Ty, span: Span) -> Result<Vec<Constraint>, Vec<GlyimDiagnostic>>`, `resolve_ty_shallow(&self, ctx: &dyn TypeLookup, ty: Ty) -> Ty`, `fully_resolve(&self, ctx: &dyn TypeLookup, ty: Ty) -> Result<Ty, Vec<TyVar>>`, `probe_ty_var(&self, var: TyVar) -> Option<Ty>`, `probe_int_var(&self, var: IntVar) -> Option<Ty>`, `probe_float_var(&self, var: FloatVar) -> Option<Ty>`, `universe(&self) -> UniverseIndex`, `create_universe(&mut self) -> UniverseIndex`
- `pub trait TraitSolver` — `can_prove(&mut self, ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult`, `evaluate_predicate(&mut self, ctx: &TyCtx, predicate: &Predicate) -> SolverResult`
- `pub enum SolverResult` — `Proven`, `Ambiguous`, `DefiniteNo`
- `pub struct SimpleTraitSolver<'a>` — `new(trait_ctx: &'a TraitContext)`
- `pub struct TraitContext` — `new()`, `register_trait(def: TraitDef)`, `register_impl(def: ImplDef)`, `impls_of_trait(&self, trait_id: TraitDefId) -> impl Iterator<Item = &ImplDef>`
- `pub struct TraitDef` — `def_id: TraitDefId`, `name: Name`, `associated_types: Vec<Name>`, `predicates: Vec<Predicate>`
- `pub struct ImplDef` — `def_id: ImplDefId`, `trait_ref: TraitRef`, `predicates: Vec<Predicate>`
- `pub struct FulfillmentCtx<'a>` — `new(ctx: &'a TyCtx, solver: &'a mut dyn TraitSolver)`, `register_obligation(&mut self, obligation: Obligation)`, `process_obligations(&mut self, limit: usize) -> Result<(), OverflowError>`, `into_diagnostics(self) -> Vec<GlyimDiagnostic>`
- `pub struct Obligation` — `predicate: Predicate`, `cause: ObligationCause`
- `pub struct ObligationCause` — `span: Span`, `code: ObligationCauseCode`
- `pub enum ObligationCauseCode` — `WellFormed`, `TypeConstruction`, `MatchArm`, `IfThenElse`
- `pub struct OverflowError` — `predicate: Predicate`, `depth: usize`
- `pub enum Constraint` — `TypeEq { a: Ty, b: Ty }`, `RegionEq { a: Region, b: Region }`, `RegionOutlives { a: Region, b: Region }`, `TypeOutlives { ty: Ty, region: Region }`
- `pub enum VariableKind` — `General`, `Integer`, `Float`

---

## glyim-typeck

- `pub fn typeck_crate(ctx: TyCtxMut, def_map: &CrateDefMap, hir: &CrateHir, solver: &mut dyn TraitSolver) -> (TyCtx, TypeckResult)`
- `pub struct TypeckResult` — `expr_types: IndexVec<glyim_hir::ExprId, Option<Ty>>`, `pat_types: IndexVec<glyim_hir::PatId, Option<Ty>>`, `adjustments: IndexVec<glyim_hir::ExprId, Option<Vec<Adjustment>>>`, `thir_bodies: Vec<(LocalDefId, thir::Body)>`, `diagnostics: Vec<GlyimDiagnostic>`
- `pub struct Adjustment` — `kind: AdjustKind`, `target: Ty`
- `pub enum AdjustKind` — `Deref`, `Borrow(Mutability)`, `NeverToAny`
- `pub struct TypeckCtx<'a>` — `ctx: &'a mut TyCtxMut`, `infer: &'a mut InferenceTable`, `diagnostics: &'a mut Vec<GlyimDiagnostic>`, `pending_obligations: Vec<Obligation>`, `unify(&mut self, a: Ty, b: Ty, span: Span) -> bool`, `require_trait_bound(&mut self, ty: Ty, trait_pred: TraitPredicate, span: Span)`
- `pub mod thir` — all types: `Body`, `Param`, `Stmt`, `Expr`, `ExprKind`, `Pattern`, `PatternKind`, `Literal`, `MatchArm`, `FieldPat`, `Capture`, `CaptureKind`, `LocalVarId`

---

## glyim-lower

- `pub trait LowerCtx` — `ty_ctx(&self) -> &TyCtx`, `adt_def(&self, id: AdtId) -> AdtDef`, `push_span(&self, span: Span)`, `pop_span(&self)`
- `pub fn lower_body(ctx: &dyn LowerCtx, thir: &thir::Body) -> LowerResult`
- `pub struct LowerResult` — `body: Body`, `diagnostics: Vec<GlyimDiagnostic>`
- `pub struct AdtDef` — `variants: Vec<AdtVariant>`, `kind: AdtKind`
- `pub struct AdtVariant` — `fields: Vec<Ty>`
- `pub enum AdtKind` — `Struct`, `Enum`, `Union`
- `pub struct MonoCtx` — `new()`, `collect(&mut self, start: &[MonoItem], mir_bodies: &dyn Fn(DefId, &Substitution) -> Arc<Body>)`, `instantiate(ctx: &TyCtx, body: &Body, substs: &Substitution) -> Body`, `items(&self) -> &[MonoItemData]`, `item_count(&self) -> usize`
- `pub enum MonoItem` — `Fn { def_id: FnDefId, substs: Substitution }`, `Const { def_id: ConstDefId, substs: Substitution }`, `Static { def_id: StaticDefId }`
- `pub struct MonoItemId` — `from_raw`, `to_raw`
- `pub struct MonoItemData` — `item: MonoItem`, `body: Arc<Body>`, `symbol: String`

---

## glyim-borrowck

- `pub trait BorrowckCtx` — `ty_ctx(&self) -> &TyCtx`, `local_decl(&self, local: LocalIdx) -> &LocalDecl`, `is_copy(&self, ty: Ty) -> bool`
- `pub fn check_borrows(ctx: &dyn BorrowckCtx, body: &Body) -> BorrowckResult`
- `pub struct BorrowckResult` — `errors: Vec<GlyimDiagnostic>`

---

## glyim-opt

- `pub fn optimize(ctx: &TyCtx, body: &Arc<Body>) -> Optimized`
- `pub struct Optimized` — `body: Body`

---

## glyim-layout

- `pub trait LayoutComputer` — `layout_of(&self, ty: Ty) -> Result<Layout, LayoutError>`, `fn_abi_of(&self, sig: &FnSig) -> Result<FnAbi, LayoutError>`, `ptr_size(&self) -> Size`, `ptr_align(&self) -> Align`, `target_info(&self) -> &TargetInfo`
- `pub struct SimpleLayoutComputer<'a>` — `new(ctx: &'a TyCtx, target: TargetInfo)`
- `pub struct Layout` — `size: Size`, `align: Align`, `fields: FieldsShape`, `variants: VariantsShape`, `is_unsized: bool`, `scalar(size: Size, align: Align) -> Self`, `unit() -> Self`
- `pub struct Size(pub u64)` — `ZERO`, `bytes(b: u64) -> Self`, `bits(&self) -> u64`, `align_to(&self, align: Align) -> Self`
- `pub struct Align(pub u64)` — `ONE`, `EIGHT`, `from_bytes(bytes: u64) -> Self`, `max(self, other: Self) -> Self`
- `pub enum LayoutError` — `UnknownType(Ty)`, `SizeOverflow(Ty)`, `Unsized(Ty)`, `Cycle(Ty)`, `AlignmentExceedsRuntime { ty: Ty, align: u64, max: u64 }`
- `pub struct FnAbi` — `args: Vec<ArgAbi>`, `ret: ArgAbi`, `conv: CallConvention`, `c_variadic: bool`
- `pub struct ArgAbi` — `ty: Ty`, `layout: Layout`, `mode: PassMode`
- `pub enum PassMode` — `Direct`, `Indirect { meta_attrs: bool }`, `Ignore`
- `pub enum CallConvention` — `Glyim`, `C`, `System`
- `pub enum FieldsShape` — `Primitive`, `Array { stride: Size, count: u64 }`, `Arbitrary { offsets: IndexVec<FieldIdx, Size> }`
- `pub enum VariantsShape` — `Single { index: u32 }`, `Multiple { tag: Ty, tag_field: u32, tag_encoding: TagEncoding, variants: Vec<Layout> }`
- `pub enum TagEncoding` — `Direct`, `Niche { untagged_variant: u32, niche_variants: std::ops::RangeInclusive<u32>, niche_start: u128 }`

---

## glyim-vfs

- `pub struct Vfs` — `new()`, `add_file_from_disk(&self, path: &Path) -> std::io::Result<FileId>`, `add_file_content(&self, path: &Path, content: Arc<str>) -> FileId`, `set_file_content(&self, file_id: FileId, content: Arc<str>)`, `file_content(&self, file_id: FileId) -> Option<Arc<str>>`, `file_content_ref<R>(&self, file_id: FileId, f: impl FnOnce(&str) -> R) -> Option<R>`, `file_path(&self, file_id: FileId) -> Option<PathBuf>`, `file_id(&self, path: &Path) -> Option<FileId>`, `len(&self) -> usize`, `is_empty(&self) -> bool`

---

## glyim-meta

- `pub enum MacroKind` — `Declarative { name: Name }`, `Proc { name: Name }`, `Builtin { name: Name, handler: BuiltinMacro }`
- `pub enum BuiltinMacro` — `File`, `Line`, `Column`, `Include`, `Env`
- `pub struct MacroDef` — `name: Name`, `kind: MacroKind`, `span: Span`
- `pub struct ExpansionResult` — `expanded: Option<SyntaxNode>`, `diagnostics: Vec<GlyimDiagnostic>`
- `pub struct Expander<'a>` — `new(hygiene: &'a mut HygieneCtx)`, `register_macro(&mut self, def: MacroDef)`, `expand(&mut self, name: Name, args: &SyntaxNode, call_site: Span) -> ExpansionResult`, `expand_crate(&mut self, root: &SyntaxNode) -> (SyntaxNode, Vec<GlyimDiagnostic>)`

---

## glyim-runtime

- `pub unsafe extern "C" fn glyim_alloc(size: usize, align: usize) -> *mut u8`
- `pub unsafe extern "C" fn glyim_dealloc(ptr: *mut u8, size: usize, align: usize)`
- `pub extern "C" fn glyim_panic(_msg: *const u8, _len: usize) -> !`
- `pub use ALIGN_MAX`

---

## glyim-cli

- `pub struct CliArgs` — `input: PathBuf`, `output: Option<PathBuf>`, `opt_level: u8`, `target: Option<String>`, `backend: String` (fields are public via `clap` derive)
- `pub fn run() -> Result<(), Vec<GlyimDiagnostic>>`

---

## glyim-lsp

- `pub struct LspState` — `new(db: Database)`, `did_open(&mut self, path: PathBuf, content: String, version: i32)`, `did_change(&mut self, path: PathBuf, content: String, version: i32)`, `did_close(&mut self, path: &PathBuf)`, `file_content(&self, path: &PathBuf) -> Option<String>`, `diagnostics_for_file(&self, path: &PathBuf) -> Vec<GlyimDiagnostic>`, `file_id(&self, path: &PathBuf) -> Option<FileId>`
- `pub mod uri` — `pub fn path_to_uri(path: &Path) -> Result<String, String>`, `pub fn uri_to_file_path(uri: &str) -> Result<PathBuf, String>`, `pub fn offset_to_position(text: &str, offset: usize) -> Result<(usize, usize), String>`

> **Change from previous contract:** Previously declared as an empty crate stub. Now provides full LSP state and URI helpers.

---

## glyim-mir-interp

- `pub struct Interpreter<'tcx>` — `new(tcx: &'tcx TyCtx)`, `with_step_limit(self, limit: usize) -> Self`, `with_recursion_limit(self, limit: usize) -> Self`, `add_function(&mut self, def_id: DefId, body: Body)`, `step_limit(&self) -> usize`, `recursion_limit(&self) -> usize`, `run_body(&mut self, body: &Body) -> InterpResult<()>`, `get_local_value(&self, local: LocalIdx) -> Option<&InterpValue>`
- `pub enum InterpError` — `TimedOut`, `StackOverflow`, `Panic(String)`
- `pub enum InterpValue` — `Int(i128)`, `Bool(bool)`, `Unit`
- `pub type InterpResult<T> = Result<T, InterpError>`

> **Change from previous contract:** Previously declared as an empty crate stub. Now provides a full MIR interpreter.

---

**Note:** This contract reflects the exact public API as of the provided codebase. Any addition, removal, or modification of these items requires a formal Change Request before being merged.
