use glyim_test::snapshot_cst;

#[test]
fn test_where_clause_single_bound() {
    let source = "fn foo<T>() where T: Clone {}";
    snapshot_cst("where_single_bound", source);
}

#[test]
fn test_where_clause_multiple_bounds() {
    let source = "fn bar<T>() where T: Clone + Send + Sync {}";
    snapshot_cst("where_multiple_bounds", source);
}

#[test]
fn test_where_clause_multiple_predicates() {
    let source = "fn baz<T, U>() where T: Clone, U: Debug {}";
    snapshot_cst("where_multiple_predicates", source);
}

#[test]
fn test_where_clause_with_lifetime() {
    let source = "fn qux<'a, T>() where T: 'a {}";
    snapshot_cst("where_lifetime_bound", source);
}
