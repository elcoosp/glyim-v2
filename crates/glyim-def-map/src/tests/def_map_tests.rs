use crate::*;
use glyim_core::def_id::CrateId;
use glyim_core::path::{Path, PathKind, PathSegment};
use glyim_core::primitives::Visibility;
use glyim_frontend::parse_to_syntax;
use glyim_span::FileId;

fn build_map(source: &str) -> (CrateDefMap, Vec<GlyimDiagnostic>) {
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    build_def_map(&parse_result.root, CrateId::from_raw(0))
}

fn root_scope(def_map: &CrateDefMap) -> &ItemScope {
    &def_map.modules[def_map.root].scope
}

fn resolve_path(def_map: &CrateDefMap, path_str: &str) -> PerNs {
    let mut kind = PathKind::Plain;
    let remaining = if path_str.starts_with("crate::") {
        kind = PathKind::Crate;
        &path_str[7..]
    } else if path_str.starts_with("super::") {
        let depth = path_str.matches("super::").count() as u32;
        kind = PathKind::Super(depth);
        &path_str[(7 * depth as usize)..]
    } else if path_str.starts_with("self::") {
        kind = PathKind::SelfPath;
        &path_str[6..]
    } else {
        path_str
    };

    let segments: Vec<PathSegment> = remaining
        .split("::")
        .filter(|s| !s.is_empty())
        .map(|s| PathSegment {
            name: def_map.interner.intern(s),
        })
        .collect();

    let path = Path { kind, segments };
    let resolver = Resolver::new(def_map, def_map.root);
    resolver.resolve_path(&path)
}

fn resolve_from(def_map: &CrateDefMap, module: ModuleId, path_str: &str) -> PerNs {
    let kind = if path_str.starts_with("super::") {
        let depth = path_str.matches("super::").count() as u32;
        PathKind::Super(depth)
    } else {
        PathKind::Plain
    };
    let remaining = match kind {
        PathKind::Super(d) => &path_str[(7 * d as usize)..],
        _ => path_str,
    };
    let segments: Vec<PathSegment> = remaining
        .split("::")
        .filter(|s| !s.is_empty())
        .map(|s| PathSegment {
            name: def_map.interner.intern(s),
        })
        .collect();
    let path = Path { kind, segments };
    let resolver = Resolver::new(def_map, module);
    resolver.resolve_path(&path)
}

#[test]
fn t01_empty_file_creates_root_module() {
    let (def_map, diags) = build_map("");
    assert!(diags.is_empty());
    let root = def_map.root;
    let data = &def_map.modules[root];
    assert!(data.parent.is_none());
    assert!(data.children.is_empty());
    assert!(data.scope.types.is_empty());
    assert!(data.scope.values.is_empty());
    assert!(data.scope.macros.is_empty());
}

#[test]
fn t02_single_fn_appears_in_scope() {
    let (def_map, diags) = build_map("fn hello() {}");
    assert!(diags.is_empty());
    let scope = root_scope(&def_map);
    assert_eq!(scope.values.len(), 1);
    let (name, _, vis, _) = scope.values[0];
    assert_eq!(name, def_map.interner.intern("hello"));
    assert_eq!(vis, Visibility::Inherited);
    assert!(scope.types.is_empty());
}

#[test]
fn t03_struct_enum_trait_impl_in_scope() {
    let (def_map, diags) = build_map(
        "
        pub struct S;
        enum E {}
        trait T {}
        impl T for S {}
    ",
    );
    assert!(diags.is_empty());
    let scope = root_scope(&def_map);
    assert_eq!(scope.types.len(), 4);
    assert!(scope.values.is_empty());
}

#[test]
fn t04_inline_module_child() {
    let (def_map, diags) = build_map(
        "
        mod foo {
            fn inner() {}
        }
    ",
    );
    assert!(diags.is_empty());
    let root = &def_map.modules[def_map.root];
    assert_eq!(root.children.len(), 1);
    let child = root.children[0].1;
    let child_data = &def_map.modules[child];
    assert_eq!(child_data.scope.values.len(), 1);
    assert_eq!(
        child_data.scope.values[0].0,
        def_map.interner.intern("inner")
    );
}

#[test]
fn t05_visibility_pub_vs_private() {
    let (def_map, diags) = build_map(
        "
        pub fn a() {}
        fn b() {}
        pub struct C;
        struct D;
    ",
    );
    assert!(diags.is_empty());
    let scope = root_scope(&def_map);
    assert_eq!(scope.values[0].2, Visibility::Public);
    assert_eq!(scope.values[1].2, Visibility::Inherited);
    assert_eq!(scope.types[0].2, Visibility::Public);
    assert_eq!(scope.types[1].2, Visibility::Inherited);
}

#[test]
fn t06_plain_path_resolution() {
    let (def_map, diags) = build_map("fn bar() {}");
    assert!(diags.is_empty());
    let result = resolve_path(&def_map, "bar");
    assert!(result.types.is_none());
    assert!(result.values.is_some());
    let (id, vis) = result.values.unwrap();
    assert!(id.to_raw() > 0);
    assert_eq!(vis, Visibility::Inherited);
}

#[test]
fn t07_self_path_resolution() {
    let (def_map, diags) = build_map("fn baz() {}");
    assert!(diags.is_empty());
    let result = resolve_path(&def_map, "self::baz");
    assert!(result.values.is_some());
}

#[test]
fn t08_super_path_resolution() {
    // super:: from a top-level module refers to the crate root
    let (def_map, diags) = build_map(
        "
        fn crate_fn() {}
        mod my_mod {
            fn mod_fn() {}
        }
    ",
    );
    assert!(diags.is_empty());
    let mod_id = def_map.modules[def_map.root].children[0].1;

    // super::crate_fn should resolve from within my_mod
    let result = resolve_from(&def_map, mod_id, "super::crate_fn");
    assert!(
        result.values.is_some(),
        "expected crate_fn to be visible via super"
    );

    // super::mod_fn should NOT exist (mod_fn is in my_mod, not root)
    let result2 = resolve_from(&def_map, mod_id, "super::mod_fn");
    assert!(result2.is_none(), "mod_fn should not be visible via super");
}

#[test]
fn t09_crate_path_resolution() {
    let (def_map, diags) = build_map(
        "
        fn top() {}
        mod m {
            fn inner() {}
        }
    ",
    );
    assert!(diags.is_empty());
    let inner_id = def_map.modules[def_map.root].children[0].1;
    let path = Path {
        kind: PathKind::Crate,
        segments: vec![PathSegment {
            name: def_map.interner.intern("top"),
        }],
    };
    let resolver = Resolver::new(&def_map, inner_id);
    let result = resolver.resolve_path(&path);
    assert!(result.values.is_some());
}

#[test]
fn t10_duplicate_name_error() {
    let (_def_map, diags) = build_map(
        "
        fn dup() {}
        fn dup() {}
    ",
    );
    assert!(!diags.is_empty());
    let has_dup_error = diags
        .iter()
        .any(|d| d.message.contains("duplicate") || d.message.contains("already"));
    assert!(has_dup_error);
}

#[test]
fn t11_unknown_name_default() {
    let (def_map, diags) = build_map("fn existing() {}");
    assert!(diags.is_empty());
    let result = resolve_path(&def_map, "nonexistent");
    assert!(result.is_none());
}

// --- Additional tests beyond the original TDD plan ---

#[test]
fn t12_duplicate_across_namespaces_allowed() {
    // A type and a value can share the same name.
    let (def_map, diags) = build_map(
        "
        fn foo() {}
        struct foo;
    ",
    );
    assert!(diags.is_empty());
    let scope = root_scope(&def_map);
    assert_eq!(scope.values.len(), 1);
    assert_eq!(scope.types.len(), 1);
    // Both have the same name
    assert_eq!(scope.values[0].0, def_map.interner.intern("foo"));
    assert_eq!(scope.types[0].0, def_map.interner.intern("foo"));
}

#[test]
fn t13_path_through_module() {
    let (def_map, diags) = build_map(
        "
        mod math {
            fn add() {}
        }
    ",
    );
    assert!(diags.is_empty());
    let result = resolve_path(&def_map, "math::add");
    assert!(result.values.is_some(), "math::add should resolve");
}

#[test]
fn t14_visibility_inherited_stored_correctly() {
    let (def_map, diags) = build_map("fn private_fn() {}");
    assert!(diags.is_empty());
    let result = resolve_path(&def_map, "private_fn");
    let (_id, vis) = result.values.expect("should resolve");
    assert_eq!(vis, Visibility::Inherited);
}

#[test]
fn t15_many_items() {
    let mut src = String::new();
    for i in 0..50 {
        src.push_str(&format!(
            "fn f{i}() {{}}
"
        ));
    }
    let (def_map, diags) = build_map(&src);
    assert!(diags.is_empty());
    let scope = root_scope(&def_map);
    assert_eq!(scope.values.len(), 50);
}

#[test]
fn t16_self_path_through_module() {
    let (def_map, diags) = build_map(
        "
        mod m {
            fn inner() {}
        }
    ",
    );
    assert!(diags.is_empty());
    // self::m::inner
    let result = resolve_path(&def_map, "self::m::inner");
    assert!(result.values.is_some(), "self::m::inner should resolve");
}

#[test]
fn t17_crate_path_through_module() {
    let (def_map, diags) = build_map(
        "
        mod util {
            pub fn helper() {}
        }
    ",
    );
    assert!(diags.is_empty());
    let result = resolve_path(&def_map, "crate::util::helper");
    assert!(
        result.values.is_some(),
        "crate::util::helper should resolve"
    );
    let (_id, vis) = result.values.unwrap();
    assert_eq!(vis, Visibility::Public);
}

#[test]
fn t18_duplicate_inside_module_detected() {
    let (_def_map, diags) = build_map(
        "
        mod inner {
            fn dup() {}
            fn dup() {}
        }
    ",
    );
    assert!(!diags.is_empty());
    let has_dup = diags
        .iter()
        .any(|d| d.message.contains("duplicate") || d.message.contains("already"));
    assert!(has_dup);
}

#[test]
fn t19_resolve_nonexistent_module_path() {
    let (def_map, diags) = build_map("fn x() {}");
    assert!(diags.is_empty());
    let result = resolve_path(&def_map, "nonexistent::x");
    assert!(result.is_none());
}
