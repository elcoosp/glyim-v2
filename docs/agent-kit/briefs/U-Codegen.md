You are implementing Stream U-Codegen: Unstub LLVM Backend for the Glyim compiler.

## Mission
Remove all stubs in `glyim-codegen-llvm` related to ABI enum layout, MIR lowering (fat string pointers, enum tags, slice projections, direct calls, byval args, pointer casts, drop glue with real sizes, StorageLive/Dead), LLVM pass pipeline mapping, and debug info. Provide fully functional implementations.

## What You Own Exclusively (DO NOT touch any other files)
- `crates/glyim-codegen-llvm/src/abi.rs`
- `crates/glyim-codegen-llvm/src/lower.rs`
- `crates/glyim-codegen-llvm/src/types.rs`
- `crates/glyim-codegen-llvm/src/passes.rs`
- `crates/glyim-codegen-llvm/src/debug.rs`
- `crates/glyim-codegen-llvm/src/tests/u_codegen.rs` (NEW FILE)
- `crates/glyim-codegen-llvm/src/tests/mod.rs` (MODIFY - safe append only)

## Exact Implementation Guide (NO STUBS ALLOWED)

### 1. `abi.rs` (Full Enum Layout)
In `FullLayoutComputer::layout_of` for `TyKind::Adt(adt_id, _)`:
- Fetch `AdtDef`. If multiple variants, compute the layout of each variant's fields. Find max size/align.
- Determine the tag size (smallest integer to fit `variants.len()`).
- Construct `VariantsShape::Multiple { tag_size, tag_align, tag_field: 0, tag_encoding: TagEncoding::Direct, variants }`.
- In `fn_abi_of`, ensure `ArgAbi` contains a flag or attribute indicating `byval` for indirect arguments.

### 2. `types.rs` (Type Mappings)
In `llvm_type_for_ty`:
- Map `TyKind::Adt` and `TyKind::Closure` to `context.struct_type(&field_types, false)`.
- Map `TyKind::Never` to `context.struct_type(&[], false)`.
- Map `TyKind::Slice` to a struct containing `{ptr_type, i64_type}` (fat pointer).

### 3. `lower.rs` (MIR Lowering)
- **`lower_const` (String):** Fix `MirConstKind::String` to return a fat pointer struct `{i8*, i64}`. Create global string, `build_insert_value` at index 0 (ptr) and index 1 (len).
- **`lower_const` (ConstRef):** Fix `MirConstKind::ConstRef` to properly initialize the global with `set_initializer` before loading.
- **`place_ptr` (Slice):** Implement `ProjectionElem::Slice` using `build_alloca` + `build_store` instead of returning null.
- **`lower_aggregate` (Enums):** Write the enum discriminant tag at the correct offset based on the layout from `FullLayoutComputer`.
- **`lower_discriminant` (Enums):** Read and decode niche tags. If `TagEncoding::Niche`, implement: `if tag == niche_start { return untagged_variant } else { return tag - niche_start + niche_variants.start() }`.
- **`lower_call` (Direct Calls):** Resolve `Operand::Constant(MirConstKind::Fn)` to a direct `FunctionValue` and use `build_call`. For indirect args, add the `byval` LLVM attribute.
- **`lower_cast` (Pointers):** Emit `builder.build_bit_cast(val, target_llvm_ty, "ptrcast")` for `CastKind::PtrToPtr` and `CastKind::FnPtrToPtr`.
- **`lower_drop` (Dealloc):** Pass real layout sizes: `let layout = FullLayoutComputer::new(...).layout_of(place_ty).unwrap();`
- **`lower_body` & `lower_statement` (Drop Glue):** Generate per-type drop glue functions (`glyim_drop_in_place_<ty>`) in `lower_body` and call them from `StorageDead`. Pass drop flags via local state.

### 4. `passes.rs` (Pipeline)
Remove the `eprintln!` debug statement. Map `(opt_level, opt_for_size)` to exact LLVM pipeline strings (e.g., `default<O2>`).

### 5. `debug.rs` (Debug Info)
Implement `declare_local` to emit `llvm.dbg.declare` intrinsics. Use `self.builder.insert_declare_at_end(alloca, local_var, None, location, block);`

## Execution Rules (MANDATORY: plan-to-cat-scripts skill)
You MUST follow the `plan-to-cat-scripts` skill exactly. Output ONLY fenced bash code blocks.

1. **Setup:** First script MUST set `STREAM_ID="U-Codegen"`, `WORKTREE_DIR="../glyim-worktrees/stream-U-Codegen"`. Use `git worktree add --detach "$WORKTREE_DIR" main`, cd into it, and `git checkout -b "stream-${STREAM_ID}/v0.1.0"`.
2. **No `#` comments:** Every action must be logged with `echo`.
3. **Heredocs:** MUST use the fixed delimiter `EOF`. Ensure no lines in the content are exactly `EOF`.
4. **Patches:** For trivial single-line replacements use `sed`. For multi-line replacements, use Python with temp files (heredocs with `EOF`). No Python string literals containing the content.
5. **Tests:** Create `crates/glyim-codegen-llvm/src/tests/u_codegen.rs` with unit tests. Use the Python safe-append pattern to add `mod u_codegen;` to `crates/glyim-codegen-llvm/src/tests/mod.rs`.
6. **Verify:** Run `cargo check --workspace` at the end. If `COMPILE_OK=true`, run tests and commit with `stream-U-Codegen: feat(codegen-llvm): unstub abi, lowering, and drop glue`.
