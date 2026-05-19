use crate::*;

#[test]
fn test_expn_id_root() {
    assert!(ExpnId::ROOT.is_root());
    assert_eq!(ExpnId::ROOT.to_raw(), 0);
}

#[test]
fn test_syntax_context_root() {
    assert!(SyntaxContext::ROOT.is_root());
    assert_eq!(SyntaxContext::ROOT.to_raw(), 0);
}

#[test]
fn test_hygiene_ctx_new_has_root_expansion() {
    let ctx = HygieneCtx::new();
    // Root expansion should be present
    assert!(ctx.expn_data(ExpnId::ROOT).is_some());
    // Any other ID should be None initially
    assert!(ctx.expn_data(ExpnId::from_raw(1)).is_none());
}

#[test]
fn test_push_expansion_adds_data() {
    let mut ctx = HygieneCtx::new();
    let data = ExpnData {
        expn_id: ExpnId::ROOT, // will be overwritten
        parent: ExpnId::ROOT,
        kind: ExpnKind::Root,
        call_site: Span::DUMMY,
        def_site: Span::DUMMY,
        transparency: Transparency::Opaque,
    };
    let id = ctx.push_expansion(data);
    assert!(!id.is_root());
    let stored = ctx.expn_data(id).unwrap();
    assert!(matches!(stored.kind, ExpnKind::Root));
    assert_eq!(stored.expn_id, id);
    // Root should still exist
    assert!(ctx.expn_data(ExpnId::ROOT).is_some());
}

#[test]
fn test_apply_remove_mark_roundtrip() {
    let mut ctx = HygieneCtx::new();
    let file = FileId::from_raw(42);
    let lo = ByteIdx::ZERO;
    let hi = ByteIdx::from_raw(10);
    let root_span = Span::new(file, lo, hi, SyntaxContext::ROOT);
    let mark = Mark {
        expn_id: ExpnId::ROOT, // use root mark for simplicity
        transparency: Transparency::Transparent,
    };
    let marked = ctx.apply_mark(root_span, mark);
    assert!(!marked.ctx.is_root());
    let (unmarked, extracted_mark) = ctx.remove_mark(marked);
    assert_eq!(extracted_mark, Some(mark));
    assert_eq!(unmarked.file, file);
    assert_eq!(unmarked.lo, lo);
    assert_eq!(unmarked.hi, hi);
    assert_eq!(unmarked.ctx, SyntaxContext::ROOT);
}

#[test]
fn test_remove_mark_on_root_returns_none() {
    let ctx = HygieneCtx::new();
    let span = Span::DUMMY;
    let (same, mark) = ctx.remove_mark(span);
    assert_eq!(same, span);
    assert_eq!(mark, None);
}

#[test]
fn test_adjust_strips_context_and_applies_scope() {
    let mut ctx = HygieneCtx::new();
    let file = FileId::from_raw(1);
    let lo = ByteIdx::ZERO;
    let hi = ByteIdx::from_raw(5);
    let root_span = Span::new(file, lo, hi, SyntaxContext::ROOT);
    let mark1 = Mark {
        expn_id: ExpnId::ROOT,
        transparency: Transparency::Opaque,
    };
    let marked1 = ctx.apply_mark(root_span, mark1);
    assert!(!marked1.ctx.is_root());

    // Create a second mark (different transparency)
    let mark2 = Mark {
        expn_id: ExpnId::ROOT,
        transparency: Transparency::Transparent,
    };
    let marked2 = ctx.apply_mark(marked1, mark2);
    assert_ne!(marked2.ctx, marked1.ctx);

    // Adjust marked2 to a scope context that is just the first mark
    // We'll use the context of marked1 as the scope_ctx
    let adjusted = ctx.adjust(marked2, marked1.ctx);
    // The adjusted span should have the same file/lo/hi as root_span and its context should be marked1.ctx
    assert_eq!(adjusted.file, file);
    assert_eq!(adjusted.lo, lo);
    assert_eq!(adjusted.hi, hi);
    assert_eq!(adjusted.ctx, marked1.ctx);
}

#[test]
fn test_adjust_on_root_span_just_changes_ctx() {
    let mut ctx = HygieneCtx::new();
    let span = Span::new(
        FileId::from_raw(10),
        ByteIdx::from_raw(0),
        ByteIdx::from_raw(8),
        SyntaxContext::ROOT,
    );
    let target_ctx = SyntaxContext::ROOT; // same
    let adjusted = ctx.adjust(span, target_ctx);
    assert_eq!(adjusted, span);
}
