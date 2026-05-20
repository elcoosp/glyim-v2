use glyim_test::snapshot_cst;

#[test]
fn test_nested_generic_shift_gt() {
    let source = "struct S<A<B<C>>>;";
    snapshot_cst("nested_generic_shift_gt", source);
}

#[test]
fn test_type_arg_list_with_shift_gt() {
    let source = "fn foo<A<B<C>>>() {}";
    snapshot_cst("fn_type_arg_shift_gt", source);
}
