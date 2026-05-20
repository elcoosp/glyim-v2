use glyim_test::snapshot_cst;

#[test]
fn test_pub_crate_visibility() {
    let source = "pub(crate) fn foo() {}";
    snapshot_cst("pub_crate_fn", source);
}

#[test]
fn test_pub_super_visibility() {
    let source = "pub(super) struct S;";
    snapshot_cst("pub_super_struct", source);
}

#[test]
fn test_pub_in_path_visibility() {
    let source = "pub(in crate::foo) mod m;";
    snapshot_cst("pub_in_path_mod", source);
}

#[test]
fn test_plain_pub_visibility() {
    let source = "pub fn bar() {}";
    snapshot_cst("plain_pub_fn", source);
}

#[test]
fn test_no_visibility() {
    let source = "fn baz() {}";
    snapshot_cst("no_visibility_fn", source);
}
