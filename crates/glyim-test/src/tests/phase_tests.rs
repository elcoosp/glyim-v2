use crate::assertions::span::{assert_span_pushed, assert_spans_balanced};
use crate::*;

#[test]
fn test_pipeline_compiler_construction() {
    let backend = mock::MockCodegen::new();
    let _compiler = harness::compiler::PipelineCompiler::new(std::sync::Arc::new(backend)
        as std::sync::Arc<dyn glyim_codegen::CodegenBackend + Send + Sync>);
}

#[test]
fn test_frontend_only_compiler() {
    use harness::compiler::TestCompiler;
    let compiler = harness::compiler::FrontendOnlyCompiler;
    let output = compiler.compile("fn main() {}", glyim_span::FileId::from_raw(9999), &[]);
    assert!(output.syntax_tree.is_some());
}

#[test]
fn test_frontend_tester() {
    let trace = FrontendTester::new("fn main() {}").run();
    assert!(trace.parse_tree.is_some());
}

#[test]
fn test_mir_assert() {
    let ctx = test_frozen_ty_ctx();
    let body = glyim_mir::Body::dummy(glyim_core::def_id::DefId::new(
        glyim_core::def_id::CrateId::from_raw(0),
        glyim_core::def_id::LocalDefId::from_raw(0),
    ));
    assert_mir(&ctx, &body)
        .block_count(1)
        .local_count(1)
        .block_terminator(glyim_mir::BasicBlockIdx::from_raw(0), "Unreachable");
}

#[test]
fn test_span_assertions() {
    use crate::mock::lower_ctx::SpanOp;
    let ops = vec![SpanOp::Push(glyim_span::Span::DUMMY), SpanOp::Pop];
    assert_spans_balanced(&ops);
    assert_span_pushed(&ops, glyim_span::Span::DUMMY);
}
