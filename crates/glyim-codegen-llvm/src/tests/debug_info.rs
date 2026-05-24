use glyim_span::{
    ByteIdx, FileId, Span, SyntaxContext, HygieneCtx, ExpnId, ExpnData, ExpnKind, Transparency,
};
use glyim_core::{Interner, Name};
use glyim_type::TyCtxMut;
use glyim_mir::Body;
use crate::debug::resolve_span_to_location;
use crate::LlvmBackend;
use std::collections::HashMap;

fn macro_name() -> Name {
    Interner::default().intern("macro")
}

#[test]
fn resolve_span_to_location_returns_call_site_for_macro_span() {
    let mut hygiene = HygieneCtx::new();
    let file_id = FileId::from_raw(1);
    let call_site = Span::new(file_id, ByteIdx::from_raw(10), ByteIdx::from_raw(20), SyntaxContext::ROOT);
    let expn_id = ExpnId::from_raw(1);
    let expn_data = ExpnData {
        expn_id,
        parent: ExpnId::ROOT,
        kind: ExpnKind::MacroRules { name: macro_name() },
        call_site,
        def_site: Span::DUMMY,
        transparency: Transparency::Transparent,
    };
    hygiene.push_expansion(expn_data);
    let macro_ctx = SyntaxContext::from_raw(expn_id.to_raw());
    let expanded_span = Span::new(file_id, ByteIdx::from_raw(15), ByteIdx::from_raw(25), macro_ctx);
    let resolved = resolve_span_to_location(expanded_span, &hygiene);
    assert_eq!(resolved.file, call_site.file);
    assert_eq!(resolved.lo.to_usize(), call_site.lo.to_usize());
    assert_eq!(resolved.hi.to_usize(), call_site.hi.to_usize());
    assert!(resolved.ctx.is_root());
}

#[test]
fn resolve_span_to_location_returns_unchanged_for_root_span() {
    let hygiene = HygieneCtx::new();
    let file_id = FileId::from_raw(1);
    let root_span = Span::new(file_id, ByteIdx::from_raw(10), ByteIdx::from_raw(20), SyntaxContext::ROOT);
    let resolved = resolve_span_to_location(root_span, &hygiene);
    assert_eq!(resolved, root_span);
}

#[test]
fn resolve_span_to_location_returns_dummy_for_dummy_span() {
    let hygiene = HygieneCtx::new();
    let resolved = resolve_span_to_location(Span::DUMMY, &hygiene);
    assert!(resolved.is_dummy());
}

#[test]
fn macro_defined_function_has_correct_line_numbers_in_ir() {
    let mut hygiene = HygieneCtx::new();
    let file_id = FileId::from_raw(1);
    let source = "line1\nline2\nline3\nline4\nline5".to_string();
    let call_site = Span::new(file_id, ByteIdx::from_raw(0), ByteIdx::from_raw(5), SyntaxContext::ROOT);
    let expn_id = ExpnId::from_raw(2);
    let expn_data = ExpnData {
        expn_id,
        parent: ExpnId::ROOT,
        kind: ExpnKind::MacroRules { name: macro_name() },
        call_site,
        def_site: Span::DUMMY,
        transparency: Transparency::Transparent,
    };
    hygiene.push_expansion(expn_data);
    let macro_ctx = SyntaxContext::from_raw(expn_id.to_raw());
    let macro_span = Span::new(file_id, ByteIdx::from_raw(10), ByteIdx::from_raw(15), macro_ctx);
    let mut source_map = HashMap::new();
    source_map.insert(file_id, ("test.g".to_string(), source.clone()));

    let mut ctx_mut = TyCtxMut::new(Interner::default());
    let unit_ty = ctx_mut.unit_ty();
    let return_ty = unit_ty;
    let mut locals = glyim_core::arena::IndexVec::new();
    locals.push(glyim_mir::LocalDecl { ty: return_ty, mutability: glyim_core::primitives::Mutability::Not, source_info: glyim_mir::SourceInfo::new(macro_span) });
    let mut basic_blocks = glyim_core::arena::IndexVec::new();
    let terminator = glyim_mir::Terminator { kind: glyim_mir::TerminatorKind::Return, source_info: glyim_mir::SourceInfo::new(macro_span) };
    basic_blocks.push(glyim_mir::BasicBlockData { statements: vec![], terminator, is_cleanup: false });
    let body = Body {
        owner: glyim_core::DefId::new(glyim_core::CrateId::from_raw(1), glyim_core::LocalDefId::from_raw(1)),
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty,
        span: macro_span,
        var_debug_info: vec![],
    };

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(source_map)
        .with_hygiene_ctx(hygiene);
    let ir = backend.generate_ir(&body).expect("IR generation failed");
    let call_site_line = source[..call_site.lo.to_usize()].matches('\n').count() + 1;
    let expected_line_pattern = format!("line: {}", call_site_line);
    assert!(ir.contains(&expected_line_pattern), "IR missing correct line number.\nIR:\n{}\nExpected line: {}", ir, call_site_line);
}

#[test]
fn nested_macro_expansions_produce_correct_inline_locations() {
    let mut hygiene = HygieneCtx::new();
    let file_id = FileId::from_raw(1);
    let source = "line1\nline2\nline3\nline4\nline5".to_string();
    // Outer macro call site at line 1
    let outer_call_site = Span::new(file_id, ByteIdx::from_raw(0), ByteIdx::from_raw(5), SyntaxContext::ROOT);
    let outer_expn_id = ExpnId::from_raw(3);
    let outer_expn_data = ExpnData {
        expn_id: outer_expn_id,
        parent: ExpnId::ROOT,
        kind: ExpnKind::MacroRules { name: macro_name() },
        call_site: outer_call_site,
        def_site: Span::DUMMY,
        transparency: Transparency::Transparent,
    };
    hygiene.push_expansion(outer_expn_data);
    let outer_ctx = SyntaxContext::from_raw(outer_expn_id.to_raw());

    // Inner macro call site inside the outer macro body (conceptually at line 2)
    let inner_call_site = Span::new(file_id, ByteIdx::from_raw(10), ByteIdx::from_raw(15), SyntaxContext::ROOT);
    let inner_expn_id = ExpnId::from_raw(4);
    let inner_expn_data = ExpnData {
        expn_id: inner_expn_id,
        parent: outer_expn_id,
        kind: ExpnKind::MacroRules { name: macro_name() },
        call_site: inner_call_site,
        def_site: Span::DUMMY,
        transparency: Transparency::Transparent,
    };
    hygiene.push_expansion(inner_expn_data);
    let inner_ctx = SyntaxContext::from_raw(inner_expn_id.to_raw());

    // Span inside the inner macro expansion
    let inner_span = Span::new(file_id, ByteIdx::from_raw(20), ByteIdx::from_raw(25), inner_ctx);
    let mut source_map = HashMap::new();
    source_map.insert(file_id, ("test.g".to_string(), source.clone()));

    // Build a body with the inner span
    let mut ctx_mut = TyCtxMut::new(Interner::default());
    let unit_ty = ctx_mut.unit_ty();
    let return_ty = unit_ty;
    let mut locals = glyim_core::arena::IndexVec::new();
    locals.push(glyim_mir::LocalDecl { ty: return_ty, mutability: glyim_core::primitives::Mutability::Not, source_info: glyim_mir::SourceInfo::new(inner_span) });
    let mut basic_blocks = glyim_core::arena::IndexVec::new();
    let terminator = glyim_mir::Terminator { kind: glyim_mir::TerminatorKind::Return, source_info: glyim_mir::SourceInfo::new(inner_span) };
    basic_blocks.push(glyim_mir::BasicBlockData { statements: vec![], terminator, is_cleanup: false });
    let body = Body {
        owner: glyim_core::DefId::new(glyim_core::CrateId::from_raw(1), glyim_core::LocalDefId::from_raw(1)),
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty,
        span: inner_span,
        var_debug_info: vec![],
    };

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(source_map)
        .with_hygiene_ctx(hygiene);
    let ir = backend.generate_ir(&body).expect("IR generation failed");
    // The resolved location should be the outer call site (line 1)
    let outer_call_site_line = source[..outer_call_site.lo.to_usize()].matches('\n').count() + 1;
    let expected_line_pattern = format!("line: {}", outer_call_site_line);
    assert!(ir.contains(&expected_line_pattern), "IR missing correct line number for nested macro expansion.\nIR:\n{}\nExpected line: {}", ir, outer_call_site_line);
}
