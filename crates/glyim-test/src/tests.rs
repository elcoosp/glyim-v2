use crate::*;
use glyim_core::primitives::*;
use glyim_type::{Ty, TyKind, Region, InferVar};
use crate::assertions::ty::assert_ty_eq;
use crate::assertions::span::{assert_span_pushed, assert_spans_balanced};

#[test]
fn test_ty_assert_is_int() {
    let (ctx, ty) = with_fresh_ty_ctx(|ctx| ctx.mk_ty(TyKind::Int(IntTy::I32)));
    assert_ty(&ctx, ty).is_int(IntTy::I32);
}

#[test]
fn test_ty_assert_is_bool() {
    let (ctx, ty) = with_fresh_ty_ctx(|ctx| ctx.bool_ty());
    assert_ty(&ctx, ty).is_bool();
}

#[test]
fn test_ty_assert_is_unit() {
    let (ctx, ty) = with_fresh_ty_ctx(|ctx| ctx.unit_ty());
    assert_ty(&ctx, ty).is_unit();
}

#[test]
fn test_ty_assert_is_never() {
    let (ctx, ty) = with_fresh_ty_ctx(|ctx| ctx.never_ty());
    assert_ty(&ctx, ty).is_never();
}

#[test]
fn test_ty_assert_is_error() {
    let ctx = test_frozen_ty_ctx();
    assert_ty(&ctx, Ty::ERROR).is_error();
}

#[test]
fn test_ty_assert_chained_ref() {
    let mut ctx_mut = test_ty_ctx();
    let inner = ctx_mut.bool_ty();
    let ref_ty = ctx_mut.mk_ref(Region::Erased, inner, Mutability::Mut);
    let ctx = ctx_mut.freeze();
    assert_ty(&ctx, ref_ty).is_ref(Mutability::Mut).is_bool();
}

#[test]
fn test_ty_assert_chained_ref_immut() {
    let mut ctx_mut = test_ty_ctx();
    let inner = ctx_mut.unit_ty();
    let ref_ty = ctx_mut.mk_ref(Region::Erased, inner, Mutability::Not);
    let ctx = ctx_mut.freeze();
    assert_ty(&ctx, ref_ty).is_ref(Mutability::Not).is_unit();
}

#[test]
fn test_ty_assert_uint() {
    let (ctx, ty) = with_fresh_ty_ctx(|ctx| ctx.mk_ty(TyKind::Uint(UintTy::U32)));
    assert_ty(&ctx, ty).is_uint(UintTy::U32);
}

#[test]
fn test_ty_assert_float() {
    let (ctx, ty) = with_fresh_ty_ctx(|ctx| ctx.mk_ty(TyKind::Float(FloatTy::F64)));
    assert_ty(&ctx, ty).is_float(FloatTy::F64);
}

#[test]
fn test_sentinel_constants() {
    let ctx = test_frozen_ty_ctx();
    assert!(matches!(ctx.ty_kind(Ty::ERROR), TyKind::Error));
    assert!(matches!(ctx.ty_kind(Ty::NEVER), TyKind::Never));
    assert!(matches!(ctx.ty_kind(Ty::UNIT), TyKind::Unit));
    assert!(matches!(ctx.ty_kind(Ty::BOOL), TyKind::Bool));
}

#[test]
fn test_check_ty_composable_ok() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    let result = check_ty(&ctx, ty).is_bool().is_not_error().finish();
    assert!(result.is_ok());
}

#[test]
fn test_check_ty_composable_fail() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    let result = check_ty(&ctx, ty).is_int(IntTy::I32).is_unit().finish();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().len(), 2);
}

#[test]
fn test_assert_ty_eq_same() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    assert_ty_eq(&ctx, ty, ty);
}

#[test]
fn test_source_builder() {
    let source = fixtures::SourceBuilder::new()
        .mode("compile-fail")
        .empty()
        .fn_def("main", "", r#"let x: i32 = "hello""#)
        .annotation("ERROR mismatched types")
        .build();
    assert!(source.contains("fn main"));
    assert!(source.contains("//~ ERROR"));
    assert!(source.contains("// test-mode: compile-fail"));
}

#[test]
fn test_source_builder_empty() {
    let source = fixtures::SourceBuilder::new().build();
    assert!(source.is_empty());
}

#[test]
fn test_property_generator_concrete() {
    let mut ctx = test_ty_ctx();
    let mut generator = property::arbitrary::Generator::new(42);
    let ty = generator.generate_ty(&mut ctx, 0);
    let frozen = ctx.freeze();
    assert!(!matches!(frozen.ty_kind(ty), TyKind::Error));
    property::arbitrary::sentinel_invariant(&frozen);
}

#[test]
fn test_property_generator_with_infer() {
    let mut ctx = test_ty_ctx();
    let mut infer = glyim_solve::InferenceTable::new();
    let mut generator = property::arbitrary::Generator::new(123);
    let ty = generator.generate_ty_with_infer(&mut ctx, &mut infer, 0);
    let frozen = ctx.freeze();
    let kind = frozen.ty_kind(ty);
    assert!(!matches!(kind, TyKind::Error));
}

#[test]
fn test_property_generator_depth_limit() {
    let mut ctx = test_ty_ctx();
    let mut generator = property::arbitrary::Generator::new(999).with_max_depth(0);
    let ty = generator.generate_ty(&mut ctx, 5);
    let frozen = ctx.freeze();
    assert!(!matches!(frozen.ty_kind(ty), TyKind::Error));
}

#[test]
fn test_unification_var_with_concrete() {
    let mut ctx = test_ty_ctx();
    let mut infer = glyim_solve::InferenceTable::new();
    let var = infer.new_ty_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)));
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    property::unify::test_unify_var_with_concrete(&mut ctx, &mut infer, var_ty, i32_ty);
}

#[test]
fn test_unification_same_type() {
    let mut ctx = test_ty_ctx();
    let mut infer = glyim_solve::InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    property::unify::test_unify_same_type_succeeds(&mut ctx, &mut infer, i32_ty);
}

#[test]
fn test_unification_different_types_fail() {
    let mut ctx = test_ty_ctx();
    let mut infer = glyim_solve::InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let bool_ty = ctx.bool_ty();
    property::unify::test_unify_different_types_fails(&mut ctx, &mut infer, i32_ty, bool_ty);
}

#[test]
fn test_mock_solver() {
    use glyim_solve::TraitSolver;
    let mut solver = mock::MockSolver::new()
        .respond_for_any(glyim_solve::SolverResult::Proven);
    assert_eq!(solver.call_count(), 0);

    let mut ctx_mut = test_ty_ctx();
    let substs = ctx_mut.intern_substitution(vec![]);
    let ctx = ctx_mut.freeze();

    let trait_pred = glyim_type::TraitPredicate {
        trait_ref: glyim_type::TraitRef {
            def_id: glyim_core::def_id::TraitDefId::from_raw(0),
            substs,
        },
        polarity: glyim_type::ImplPolarity::Positive,
    };
    let result = solver.can_prove(&ctx, &trait_pred);
    assert!(matches!(result, glyim_solve::SolverResult::Proven));
    assert_eq!(solver.call_count(), 1);
}

#[test]
fn test_mock_solver_default_ambiguous() {
    use glyim_solve::TraitSolver;
    let mut solver = mock::MockSolver::new();

    let mut ctx_mut = test_ty_ctx();
    let substs = ctx_mut.intern_substitution(vec![]);
    let ctx = ctx_mut.freeze();

    let trait_pred = glyim_type::TraitPredicate {
        trait_ref: glyim_type::TraitRef {
            def_id: glyim_core::def_id::TraitDefId::from_raw(99),
            substs,
        },
        polarity: glyim_type::ImplPolarity::Positive,
    };
    let result = solver.can_prove(&ctx, &trait_pred);
    assert!(matches!(result, glyim_solve::SolverResult::Ambiguous));
}

#[test]
fn test_mock_codegen() {
    use glyim_codegen::CodegenBackend;
    let mock = mock::MockCodegen::new();
    assert_eq!(mock.name(), "mock");
    assert_eq!(mock.calls().len(), 0);
    assert_eq!(mock.function_call_count(), 0);
}

#[test]
fn test_mock_codegen_generate() {
    use glyim_codegen::CodegenBackend;
    let mock = mock::MockCodegen::new();
    let body = std::sync::Arc::new(glyim_mir::Body::dummy(
        glyim_core::def_id::DefId::new(
            glyim_core::def_id::CrateId::from_raw(0),
            glyim_core::def_id::LocalDefId::from_raw(0),
        ),
    ));
    let result = mock.generate(&[body], std::path::Path::new("test.o"));
    assert!(result.is_ok());
    assert_eq!(mock.calls().len(), 1);
    assert_eq!(mock.calls()[0].body_count, 1);
}

#[test]
fn test_mock_codegen_generate_function() {
    use glyim_codegen::CodegenBackend;
    let mock = mock::MockCodegen::new();
    let body = std::sync::Arc::new(glyim_mir::Body::dummy(
        glyim_core::def_id::DefId::new(
            glyim_core::def_id::CrateId::from_raw(0),
            glyim_core::def_id::LocalDefId::from_raw(0),
        ),
    ));
    let result = mock.generate_function(&body);
    assert!(result.is_ok());
    assert_eq!(mock.function_call_count(), 1);
}

#[test]
fn test_mock_lower_ctx() {
    use glyim_lower::LowerCtx;
    let ctx = test_frozen_ty_ctx();
    let mock = mock::MockLowerCtx::new(&ctx);
    mock.push_span(glyim_span::Span::DUMMY);
    mock.pop_span();
    mock.assert_spans_balanced();
    assert_eq!(mock.span_ops().len(), 2);
}

#[test]
fn test_mock_lower_ctx_unbalanced() {
    use glyim_lower::LowerCtx;
    let ctx = test_frozen_ty_ctx();
    let mock = mock::MockLowerCtx::new(&ctx);
    mock.push_span(glyim_span::Span::DUMMY);
    assert_eq!(mock.span_ops().len(), 1);
    let ops = mock.span_ops();
    let depth: usize = ops.iter().fold(0, |acc: usize, op| match op {
        crate::mock::lower_ctx::SpanOp::Push(_) => acc + 1,
        crate::mock::lower_ctx::SpanOp::Pop => acc.saturating_sub(1),
    });
    assert_eq!(depth, 1);
}

#[test]
fn test_annotation_parser_exact_vs_fuzzy() {
    let exact_src = "fn main() {} //~ ERROR msg";
    let anns = annotations::Annotation::parse_all(exact_src).unwrap();
    assert_eq!(anns.len(), 1);
    assert!(!anns[0].fuzzy, "//~ must be exact");

    let fuzzy_src = "fn main() {} //~~ ERROR msg";
    let anns = annotations::Annotation::parse_all(fuzzy_src).unwrap();
    assert_eq!(anns.len(), 1);
    assert!(anns[0].fuzzy, "//~~ must be fuzzy");
}

#[test]
fn test_annotation_caret_offset() {
    let src = "fn main() {} //~^^ ERROR msg";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns.len(), 1);
    assert_eq!(anns[0].line_offset, 2);
    assert_eq!(anns[0].target_line(), 0);
}

#[test]
fn test_annotation_optional() {
    let src = "fn main() {} //~? ERROR msg";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns.len(), 1);
    assert!(anns[0].optional);
}

#[test]
fn test_annotation_continuation() {
    let src = "line1\nline2\nline3 //~ ERROR msg\n     //~| NOTE sub";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns.len(), 2);
    assert_eq!(anns[0].target_line(), anns[1].target_line());
}

#[test]
fn test_annotation_severity_default_error() {
    let src = "fn main() {} //~ msg";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns[0].severity, glyim_diag::DiagSeverity::Error);
}

#[test]
fn test_annotation_severity_warning() {
    let src = "fn main() {} //~ WARNING msg";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns[0].severity, glyim_diag::DiagSeverity::Warning);
}

#[test]
fn test_annotation_severity_note() {
    let src = "fn main() {} //~ NOTE msg";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns[0].severity, glyim_diag::DiagSeverity::Note);
}

#[test]
fn test_annotation_severity_help() {
    let src = "fn main() {} //~ HELP msg";
    let anns = annotations::Annotation::parse_all(src).unwrap();
    assert_eq!(anns[0].severity, glyim_diag::DiagSeverity::Help);
}

#[test]
fn test_annotation_invalid_severity_treated_as_message() {
    let anns = annotations::Annotation::parse_all("fn main() {} //~ ERRR msg").unwrap();
    assert_eq!(anns.len(), 1);
    assert_eq!(anns[0].severity, glyim_diag::DiagSeverity::Error);
    assert!(matches!(anns[0].pattern, annotations::pattern::MatchPattern::Substring(_)));
    assert_eq!(anns[0].pattern.description(), "contains \"ERRR msg\"");
}

#[test]
fn test_comparison_invariant_with_optional() {
    use comparison;
    use annotations::Annotation;
    use annotations::pattern::MatchPattern;
    use glyim_diag::DiagSeverity;

    let ann = Annotation {
        line: 0, line_offset: 0, severity: DiagSeverity::Error,
        pattern: MatchPattern::Any, optional: true, fuzzy: false,
    };
    let result = comparison::compare_diagnostics(&[ann], &[]);
    assert!(result.passed());
    assert_eq!(result.optional_unmatched.len(), 1);
}

#[test]
fn test_comparison_exact_match() {
    use comparison;
    use annotations::Annotation;
    use annotations::pattern::MatchPattern;
    use glyim_diag::DiagSeverity;

    let ann = Annotation {
        line: 0, line_offset: 0, severity: DiagSeverity::Error,
        pattern: MatchPattern::Any, optional: false, fuzzy: false,
    };
    let diag = comparison::NormalizedDiag {
        severity: DiagSeverity::Error,
        line: 0,
        message: "test".into(),
    };
    let result = comparison::compare_diagnostics(&[ann], &[diag]);
    assert!(result.passed());
    assert_eq!(result.matched.len(), 1);
}

#[test]
fn test_comparison_wrong_severity() {
    use comparison;
    use annotations::Annotation;
    use annotations::pattern::MatchPattern;
    use glyim_diag::DiagSeverity;

    let ann = Annotation {
        line: 0, line_offset: 0, severity: DiagSeverity::Error,
        pattern: MatchPattern::Any, optional: false, fuzzy: false,
    };
    let diag = comparison::NormalizedDiag {
        severity: DiagSeverity::Warning,
        line: 0,
        message: "test".into(),
    };
    let result = comparison::compare_diagnostics(&[ann], &[diag]);
    assert!(!result.passed());
    assert_eq!(result.wrong_severity.len(), 1);
}

#[test]
fn test_comparison_fuzzy_match() {
    use comparison;
    use annotations::Annotation;
    use annotations::pattern::MatchPattern;
    use glyim_diag::DiagSeverity;

    let ann = Annotation {
        line: 5, line_offset: 0, severity: DiagSeverity::Error,
        pattern: MatchPattern::Any, optional: false, fuzzy: true,
    };
    let diag = comparison::NormalizedDiag {
        severity: DiagSeverity::Error,
        line: 6,
        message: "test".into(),
    };
    let result = comparison::compare_diagnostics(&[ann], &[diag]);
    assert!(result.passed());
}

#[test]
fn test_comparison_unexpected_diagnostic() {
    use comparison;
    use glyim_diag::DiagSeverity;

    let diag = comparison::NormalizedDiag {
        severity: DiagSeverity::Error,
        line: 0,
        message: "unexpected".into(),
    };
    let result = comparison::compare_diagnostics(&[], &[diag]);
    assert!(!result.passed());
    assert_eq!(result.unexpected.len(), 1);
}

#[test]
fn test_comparison_substring_pattern() {
    use comparison;
    use annotations::Annotation;
    use annotations::pattern::MatchPattern;
    use glyim_diag::DiagSeverity;

    let ann = Annotation {
        line: 0, line_offset: 0, severity: DiagSeverity::Error,
        pattern: MatchPattern::substring("mismatch"), optional: false, fuzzy: false,
    };
    let diag = comparison::NormalizedDiag {
        severity: DiagSeverity::Error,
        line: 0,
        message: "type mismatch: expected i32".into(),
    };
    let result = comparison::compare_diagnostics(&[ann], &[diag]);
    assert!(result.passed());
}

#[test]
fn test_test_db_builder() {
    use std::sync::Arc;
    let _db = mock::TestDbBuilder::new()
        .name("my_test")
        .target_triple("aarch64-unknown-linux-gnu")
        .opt_level(2)
        .file(std::path::PathBuf::from("main.g"), Arc::from("fn main() {}"))
        .build();
}

#[test]
fn test_test_db_builder_default() {
    let _db = mock::TestDbBuilder::default().build();
}

#[test]
fn test_layout_assertion_bool() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    assert_layout(&ctx, ty, 1, 1);
}

#[test]
fn test_layout_assertion_i32() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I32)));
    assert_layout(&ctx, ty, 4, 4);
}

#[test]
fn test_layout_assertion_u8() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Uint(UintTy::U8)));
    assert_layout(&ctx, ty, 1, 1);
}

#[test]
fn test_layout_assertion_f64() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Float(FloatTy::F64)));
    assert_layout(&ctx, ty, 8, 8);
}

#[test]
fn test_layout_assertion_unit() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.unit_ty());
    assert_layout(&ctx, ty, 0, 1);
}

#[test]
fn test_layout_assertion_never() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.never_ty());
    assert_layout(&ctx, ty, 0, 1);
}

#[test]
fn test_layout_ref_size() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| {
        let inner = c.bool_ty();
        c.mk_ref(Region::Erased, inner, Mutability::Not)
    });
    assert_layout(&ctx, ty, 8, 8);
}

#[test]
fn test_check_ty_property() {
    let result = check_ty_property(42, 50, |_ctx, _ty| Ok(()));
    assert!(result.is_ok());
}

#[test]
fn test_check_ty_property_failure() {
    let result = check_ty_property(42, 10, |_ctx, _ty| Err("bad type".to_string()));
    assert!(result.is_err());
}

#[test]
fn test_pipeline_compiler_construction() {
    let backend = mock::MockCodegen::new();
    let _compiler = harness::compiler::PipelineCompiler::new(
        std::sync::Arc::new(backend) as std::sync::Arc<dyn glyim_codegen::CodegenBackend + Send + Sync>
    );
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
fn test_diag_assertions() {
    let diags = vec![
        glyim_diag::GlyimDiagnostic::type_error(
            glyim_span::Span::DUMMY, "test error"
        ),
        glyim_diag::GlyimDiagnostic::new(
            glyim_diag::ErrorCode { category: glyim_diag::ErrorCategory::Type, number: 2 },
            glyim_diag::DiagSeverity::Warning,
            "test warning",
            glyim_diag::MultiSpan::from_span(glyim_span::Span::DUMMY),
        ),
    ];
    assert_has_errors(&diags);
    assert_error_count(&diags, 1);
    assert_diag_contains(&diags, "test error");
    assert_has_severity(&diags, glyim_diag::DiagSeverity::Warning);
}

#[test]
fn test_assert_no_errors() {
    let diags = vec![
        glyim_diag::GlyimDiagnostic::new(
            glyim_diag::ErrorCode { category: glyim_diag::ErrorCategory::Type, number: 1 },
            glyim_diag::DiagSeverity::Warning,
            "just a warning",
            glyim_diag::MultiSpan::from_span(glyim_span::Span::DUMMY),
        ),
    ];
    assert_no_errors(&diags);
}

#[test]
fn test_config_parsing() {
    let source = "// test-mode: compile-fail\n// error-pattern: mismatched types\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert!(result.has_explicit_mode);
    assert_eq!(result.config.mode, harness::config::TestMode::CompileFail);
    assert_eq!(result.config.error_patterns.len(), 1);
    assert_eq!(result.config.error_patterns[0], "mismatched types");
}

#[test]
fn test_config_default_mode() {
    let source = "fn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert!(!result.has_explicit_mode);
    assert_eq!(result.config.mode, harness::config::TestMode::CompilePass);
}

#[test]
fn test_config_ignore() {
    let source = "// ignore\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert!(result.config.ignore);
}

#[test]
fn test_config_timeout() {
    let source = "// timeout: 120\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert_eq!(result.config.timeout_secs, 120);
}

#[test]
fn test_config_compile_flags() {
    let source = "// compile-flags: --emit=mir -Zdump-mir=all\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert_eq!(result.config.compile_flags.len(), 2);
}

#[test]
fn test_config_revisions() {
    let source = "// revisions: a b c\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert_eq!(result.config.revisions, vec!["a", "b", "c"]);
}

#[test]
fn test_config_revision_flags() {
    let source = "// revisions: a b\n//[a] compile-flags: -Dwarnings\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert!(result.config.revision_compile_flags.contains_key("a"));
}

#[test]
fn test_test_mode_from_str() {
    assert_eq!("compile-pass".parse::<harness::config::TestMode>().unwrap(), harness::config::TestMode::CompilePass);
    assert_eq!("compile-fail".parse::<harness::config::TestMode>().unwrap(), harness::config::TestMode::CompileFail);
    assert_eq!("ui".parse::<harness::config::TestMode>().unwrap(), harness::config::TestMode::Ui);
    assert!("invalid".parse::<harness::config::TestMode>().is_err());
}

#[test]
fn test_test_mode_dir_name() {
    assert_eq!(harness::config::TestMode::CompilePass.dir_name(), "compile-pass");
    assert_eq!(harness::config::TestMode::CompileFail.dir_name(), "compile-fail");
    assert_eq!(harness::config::TestMode::Ui.dir_name(), "ui");
}

#[test]
fn test_normalize_output() {
    let rules = comparison::normalize::NormalizeRules {
        normalize_line_endings: true,
        normalize_slashes: true,
        substitute_dir: false,
    };
    let result = comparison::normalize::normalize_output(
        "hello\r\nworld\\path",
        std::path::Path::new("test.g"),
        &rules,
    );
    assert_eq!(result, "hello\nworld/path");
}

#[test]
fn test_normalize_substitute_dir() {
    let rules = comparison::normalize::NormalizeRules {
        normalize_line_endings: false,
        normalize_slashes: false,
        substitute_dir: true,
    };
    let dir = std::path::Path::new("/some/dir/test.g");
    let result = comparison::normalize::normalize_output(
        "error at /some/dir/file.g",
        dir,
        &rules,
    );
    assert!(result.contains("$DIR"));
}

#[test]
fn test_match_pattern_any() {
    let p = annotations::pattern::MatchPattern::Any;
    assert!(p.matches("anything"));
    assert_eq!(p.description(), "<any>");
}

#[test]
fn test_match_pattern_substring() {
    let p = annotations::pattern::MatchPattern::substring("hello");
    assert!(p.matches("say hello world"));
    assert!(!p.matches("say world"));
}

#[test]
fn test_match_pattern_exact() {
    let p = annotations::pattern::MatchPattern::exact("hello");
    assert!(p.matches("hello"));
    assert!(!p.matches("hello world"));
}

#[test]
fn test_match_pattern_regex() {
    let p = annotations::pattern::MatchPattern::regex("error|warning").unwrap();
    assert!(p.matches("got an error here"));
    assert!(p.matches("got a warning here"));
    assert!(!p.matches("got a note here"));
}

#[test]
fn test_diag_severity_ext() {
    use comparison::DiagSeverityExt;
    assert_eq!(glyim_diag::DiagSeverity::Error.display_name(), "ERROR");
    assert_eq!(glyim_diag::DiagSeverity::Warning.display_name(), "WARNING");
    assert_eq!(glyim_diag::DiagSeverity::Note.display_name(), "NOTE");
    assert_eq!(glyim_diag::DiagSeverity::Help.display_name(), "HELP");
}

#[test]
fn test_failure_reason_display() {
    let reason = error::FailureReason::TimeoutExceeded { timeout_secs: 30 };
    assert!(reason.to_string().contains("30"));
    let reason = error::FailureReason::ErrorPatternNotFound { pattern: "test".into() };
    assert!(reason.to_string().contains("test"));
}

#[test]
fn test_timeout_error() {
    let err = error::TimeoutError { timeout_secs: 60 };
    assert!(err.to_string().contains("60"));
}

#[test]
fn test_ty_factory() {
    let mut ctx = test_ty_ctx();
    let b = fixtures::TyFactory::bool(&mut ctx);
    let i = fixtures::TyFactory::i32(&mut ctx);
    let u = fixtures::TyFactory::u32(&mut ctx);
    let f = fixtures::TyFactory::f64(&mut ctx);
    let n = fixtures::TyFactory::never(&mut ctx);
    let un = fixtures::TyFactory::unit(&mut ctx);
    let r = fixtures::TyFactory::ref_to(&mut ctx, b, Mutability::Not);
    let s = fixtures::TyFactory::slice_of(&mut ctx, i);
    let frozen = ctx.freeze();
    assert!(matches!(frozen.ty_kind(b), TyKind::Bool));
    assert!(matches!(frozen.ty_kind(i), TyKind::Int(IntTy::I32)));
    assert!(matches!(frozen.ty_kind(u), TyKind::Uint(UintTy::U32)));
    assert!(matches!(frozen.ty_kind(f), TyKind::Float(FloatTy::F64)));
    assert!(matches!(frozen.ty_kind(n), TyKind::Never));
    assert!(matches!(frozen.ty_kind(un), TyKind::Unit));
    assert!(matches!(frozen.ty_kind(r), TyKind::Ref(_, _, Mutability::Not)));
    assert!(matches!(frozen.ty_kind(s), TyKind::Slice(_)));
}

#[test]
fn test_mir_assert() {
    let ctx = test_frozen_ty_ctx();
    let body = glyim_mir::Body::dummy(
        glyim_core::def_id::DefId::new(
            glyim_core::def_id::CrateId::from_raw(0),
            glyim_core::def_id::LocalDefId::from_raw(0),
        ),
    );
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

#[test]
fn test_run_pass_mode_from_str() {
    assert_eq!("run-pass".parse::<harness::config::TestMode>().unwrap(), harness::config::TestMode::RunPass);
    assert_eq!("run-fail".parse::<harness::config::TestMode>().unwrap(), harness::config::TestMode::RunFail);
}

#[test]
fn test_run_pass_mode_dir_name() {
    assert_eq!(harness::config::TestMode::RunPass.dir_name(), "run-pass");
    assert_eq!(harness::config::TestMode::RunFail.dir_name(), "run-fail");
}

#[test]
fn test_config_check_stdout() {
    let source = "// test-mode: run-pass\n// check-stdout: Hello, world!\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert_eq!(result.config.mode, harness::config::TestMode::RunPass);
    assert_eq!(result.config.check_stdout.as_deref(), Some("Hello, world!"));
}

#[test]
fn test_config_check_stderr() {
    let source = "// test-mode: run-fail\n// check-stderr: panic\n// exit-code: 101\nfn main() {}";
    let result = harness::config::parse_test_config(source).unwrap();
    assert_eq!(result.config.mode, harness::config::TestMode::RunFail);
    assert_eq!(result.config.check_stderr.as_deref(), Some("panic"));
    assert_eq!(result.config.expected_exit_code, Some(101));
}

#[test]
fn test_output_check_exit_code_pass() {
    let check = harness::runner::OutputCheck::new().exit_code(0);
    let result = harness::runner::RunResult {
        exit_code: Some(0),
        stdout: String::new(),
        stderr: String::new(),
        timed_out: false,
        duration: std::time::Duration::from_secs(0),
    };
    assert!(check.check(&result).is_ok());
}

#[test]
fn test_output_check_exit_code_fail() {
    let check = harness::runner::OutputCheck::new().exit_code(0);
    let result = harness::runner::RunResult {
        exit_code: Some(1),
        stdout: String::new(),
        stderr: String::new(),
        timed_out: false,
        duration: std::time::Duration::from_secs(0),
    };
    assert!(check.check(&result).is_err());
}

#[test]
fn test_output_check_stdout_pass() {
    let check = harness::runner::OutputCheck::new().stdout("hello");
    let result = harness::runner::RunResult {
        exit_code: Some(0),
        stdout: "say hello world".to_string(),
        stderr: String::new(),
        timed_out: false,
        duration: std::time::Duration::from_secs(0),
    };
    assert!(check.check(&result).is_ok());
}

#[test]
fn test_output_check_stdout_fail() {
    let check = harness::runner::OutputCheck::new().stdout("goodbye");
    let result = harness::runner::RunResult {
        exit_code: Some(0),
        stdout: "say hello world".to_string(),
        stderr: String::new(),
        timed_out: false,
        duration: std::time::Duration::from_secs(0),
    };
    assert!(check.check(&result).is_err());
}

#[test]
fn test_output_check_stderr_pass() {
    let check = harness::runner::OutputCheck::new().stderr("error:");
    let result = harness::runner::RunResult {
        exit_code: Some(1),
        stdout: String::new(),
        stderr: "error: something went wrong".to_string(),
        timed_out: false,
        duration: std::time::Duration::from_secs(0),
    };
    assert!(check.check(&result).is_ok());
}

#[test]
fn test_output_check_timeout() {
    let check = harness::runner::OutputCheck::new().exit_code(0);
    let result = harness::runner::RunResult {
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        timed_out: true,
        duration: std::time::Duration::from_secs(60),
    };
    let err = check.check(&result).unwrap_err();
    assert!(matches!(err, error::FailureReason::RunTimeout { .. }));
}

#[test]
fn test_output_check_run_fail_pass() {
    let check = harness::runner::OutputCheck::new().exit_code(1).stderr("panic");
    let result = harness::runner::RunResult {
        exit_code: Some(1),
        stdout: String::new(),
        stderr: "thread panicked: panic at core.rs:42".to_string(),
        timed_out: false,
        duration: std::time::Duration::from_millis(100),
    };
    assert!(check.check(&result).is_ok());
}

#[test]
fn test_program_runner_nonexistent() {
    let runner = harness::runner::ProgramRunner::new("/nonexistent/program");
    let result = runner.run(std::time::Duration::from_secs(5));
    assert!(result.exit_code.is_none());
    assert!(!result.stderr.is_empty());
}

#[test]
fn test_program_runner_echo() {
    let echo_path = if cfg!(target_os = "macos") { "/bin/echo" } else { "/bin/echo" };
    let runner = harness::runner::ProgramRunner::new(echo_path).arg("hello world");
    let result = runner.run(std::time::Duration::from_secs(5));
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, Some(0));
    assert!(result.stdout.contains("hello world"));
}

#[test]
fn test_program_runner_false() {
    let false_path = if cfg!(target_os = "macos") { "/usr/bin/false" } else { "/bin/false" };
    let runner = harness::runner::ProgramRunner::new(false_path);
    let result = runner.run(std::time::Duration::from_secs(5));
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, Some(1));
}

#[test]
fn test_program_runner_with_stdin() {
    let cat_path = if cfg!(target_os = "macos") { "/bin/cat" } else { "/bin/cat" };
    let runner = harness::runner::ProgramRunner::new(cat_path).stdin("input data");
    let result = runner.run(std::time::Duration::from_secs(5));
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, Some(0));
    assert!(result.stdout.contains("input data"));
}

#[test]
fn test_failure_reason_run_failed_display() {
    let reason = error::FailureReason::RunFailed {
        exit_code: Some(139),
        expected_exit_code: Some(0),
    };
    assert!(reason.to_string().contains("139"));
    let reason = error::FailureReason::StdoutMismatch {
        expected: "hello".to_string(),
        actual: "world".to_string(),
    };
    assert!(reason.to_string().contains("hello"));
    let reason = error::FailureReason::StderrMismatch {
        expected: "error".to_string(),
        actual: "warning".to_string(),
    };
    assert!(reason.to_string().contains("error"));
    let reason = error::FailureReason::RunTimeout { timeout_secs: 30 };
    assert!(reason.to_string().contains("30"));
}
