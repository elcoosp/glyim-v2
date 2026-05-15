use glyim_test::assert_diag_contains;

#[test]
fn unresolved_projection_diagnostic() {
    // This test will be filled once projection diagnostics are implemented.
    // For now, it just checks that the helper function exists.
    let diags: Vec<glyim_diag::GlyimDiagnostic> = vec![
        glyim_diag::GlyimDiagnostic::type_error(
            glyim_span::Span::DUMMY,
            "cannot resolve projection",
        )
    ];
    assert_diag_contains(&diags, "cannot resolve projection");
}
