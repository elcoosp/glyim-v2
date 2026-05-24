use glyim_span::{
    ByteIdx, FileId, Span, SyntaxContext, HygieneCtx, ExpnId, ExpnData, ExpnKind, Mark, Transparency,
};
use glyim_core::Name;
use glyim_type::TyCtxMut;
use glyim_mir::Body;
use crate::debug::resolve_span_to_location;
use crate::LlvmBackend;
use std::collections::HashMap;

fn create_hygiene_ctx_with_expansion() -> (HygieneCtx, Span, Span) {
    let mut hygiene = HygieneCtx::new();
    let file_id = FileId::from_raw(1);
    let call_site = Span::new(file_id, ByteIdx::from_raw(10), ByteIdx::from_raw(20), SyntaxContext::ROOT);
    let def_site = Span::new(file_id, ByteIdx::from_raw(30), ByteIdx::from_raw(40), SyntaxContext::ROOT);
    let expn_id = ExpnId::ROOT;
    let expn_data = ExpnData {
        expn_id,
        parent: ExpnId::ROOT,
        kind: ExpnKind::MacroRules { name: Name::from_raw(1) },
        call_site,
        def_site,
        transparency: Transparency::Transparent,
    };
    hygiene.push_expansion(expn_data);
    let expanded_span = Span::new(file_id, ByteIdx::from_raw(15), ByteIdx::from_raw(25), SyntaxContext::ROOT);
    let mark = Mark { expn_id, transparency: Transparency::Transparent };
    let adjusted_span = hygiene.adjust(expanded_span, &mark);
    (hygiene, adjusted_span, call_site)
}

#[test]
fn resolve_span_to_location_returns_call_site_for_macro_span() {
    let (hygiene, expanded_span, call_site) = create_hygiene_ctx_with_expansion();
    let resolved = resolve_span_to_location(expanded_span, &hygiene);
    assert_eq!(resolved.file, call_site.file);
    assert_eq!(resolved.lo.to_usize(), call_site.lo.to_usize());
    assert_eq!(resolved.hi.to_usize(), call_site.hi.to_usize());
    assert!(resolved.ctx.is_root());
}

#[test]
fn macro_defined_function_has_correct_line_numbers_in_ir() {
    // Create a hygiene context with a macro expansion that points to a call site.
    let (hygiene, macro_span, call_site) = create_hygiene_ctx_with_expansion();
    let file_id = FileId::from_raw(1);
    let source = "line1\nline2\nline3\nline4\nline5".to_string();
    let mut source_map = HashMap::new();
    source_map.insert(file_id, ("test.g".to_string(), source.clone()));
    // Create a dummy MIR body with a span that is the macro span.
    // We'll build a minimal body (just a return).
    let mut ctx_mut = TyCtxMut::new(glyim_core::Interner::default());
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
    // Create backend with debug info enabled, source map, and hygiene context.
    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(source_map)
        .with_hygiene_ctx(hygiene);
    let ir = backend.generate_ir(&body).expect("IR generation failed");
    // Check that the DILocation line number equals the call site line.
    let call_site_line = source[..call_site.lo.to_usize()].matches('\n').count() + 1;
    // The IR should contain: !DILocation(line: <call_site_line>, ...
    let expected_line_pattern = format!("line: {}", call_site_line);
    assert!(ir.contains(&expected_line_pattern), "IR missing correct line number.\nIR:\n{}\nExpected line: {}", ir, call_site_line);
}

#[test]
fn nested_macro_expansions_produce_correct_inline_locations() {
    // Simulate two levels of macro expansion.
    let mut hygiene = HygieneCtx::new();
    let file_id = FileId::from_raw(1);
    let outer_call_site = Span::new(file_id, ByteIdx::from_raw(5), ByteIdx::from_raw(15), SyntaxContext::ROOT);
    let outer_expn_id = ExpnId::from_raw(42);
    let outer_expn_data = ExpnData {
        expn_id: outer_expn_id,
        parent: ExpnId::ROOT,
        kind: ExpnKind::MacroRules { name: Name::from_raw(1) },
        call_site: outer_call_site,
        def_site: Span::DUMMY,
        transparency: Transparency::Transparent,
    };
    hygiene.push_expansion(outer_expn_data);
    let inner_call_site = Span::new(file_id, ByteIdx::from_raw(20), ByteIdx::from_raw(30), SyntaxContext::ROOT);
    let inner_expn_id = ExpnId::from_raw(43);
    let inner_expn_data = ExpnData {
        expn_id: inner_expn_id,
        parent: outer_expn_id,
        kind: ExpnKind::MacroRules { name: Name::from_raw(2) },
        call_site: inner_call_site,
        def_site: Span::DUMMY,
        transparency: Transparency::Transparent,
    };
    hygiene.push_expansion(inner_expn_data);
    let inner_mark = Mark { expn_id: inner_expn_id, transparency: Transparency::Transparent };
    let outer_mark = Mark { expn_id: outer_expn_id, transparency: Transparency::Transparent };
    // Create a span inside the inner macro body, then apply both marks.
    let inner_span = Span::new(file_id, ByteIdx::from_raw(25), ByteIdx::from_raw(35), SyntaxContext::ROOT);
    let inner_expanded = hygiene.adjust(inner_span, &inner_mark);
    let outer_expanded = hygiene.adjust(inner_expanded, &outer_mark);
    // Resolve should walk back to outer_call_site, then to the original root? Actually resolve_span_to_location walks until root.
    let resolved = resolve_span_to_location(outer_expanded, &hygiene);
    // The final resolved span should be the outermost call site (outer_call_site) because that's the root.
    assert_eq!(resolved.file, outer_call_site.file);
    assert_eq!(resolved.lo.to_usize(), outer_call_site.lo.to_usize());
    // Also ensure the resolved span's context is root.
    assert!(resolved.ctx.is_root());
}
