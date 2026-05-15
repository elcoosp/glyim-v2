use glyim_syntax::SyntaxNode;
use glyim_core::def_id::CrateId;
use glyim_core::interner::Name;
use glyim_core::path::PathKind;
use glyim_core::primitives::Visibility;
use glyim_test::FrontendTester;


// Helper to debug CST
fn dump_cst(node: &SyntaxNode, indent: usize) {
    println!("{}{:?} '{}'", "  ".repeat(indent), node.kind(), node.text().to_string().chars().take(30).collect::<String>());
    for child in node.children() {
        dump_cst(&child, indent + 1);
    }
}

fn get_def_map(source: &str) -> (crate::CrateDefMap, Vec<glyim_diag::GlyimDiagnostic>) {
    let trace = FrontendTester::new(source).run();

    // Print parse diagnostics to debug parser issues
    if !trace.parse_diagnostics.is_empty() {
        println!("--- PARSE DIAGNOSTICS ---");
        for d in &trace.parse_diagnostics {
            println!("{:?}", d);
        }
        println!("--- END PARSE DIAGNOSTICS ---");
    }

    let root = trace.parse_tree.expect("Parse tree should exist");
    crate::build_def_map(&root, CrateId::from_raw(0))
}

#[test]
fn u08_t01_std_io_read_imports_read() {
    println!("=== CST DUMP FOR u08_t01 ===");
    let source = r#"
        mod std {
            pub mod io {
                pub struct Read;
            }
        }
        use std::io::Read;
    "#;
    let trace = FrontendTester::new(source).run();
    let root = trace.parse_tree.expect("Parse tree should exist");
    dump_cst(&root, 0);
    println!("=== END CST DUMP ===
");

    // Setup: Define a fake std hierarchy
    let source = r#"
        mod std {
            pub mod io {
                pub struct Read;
            }
        }
        use std::io::Read;
    "#;
    let (def_map, _) = get_def_map(source);

    // Resolve 'Read' in the root module
    let root_mod = def_map.root;
    let read_name = def_map.interner.intern("Read");

    let resolved = def_map.modules[root_mod].resolve(read_name);
    assert!(resolved.is_some(), "Read should be in root scope");

    let (id, vis) = resolved.unwrap();
    assert_eq!(vis, Visibility::Public, "Read should be public");
    // We don't check the exact LocalDefId because it depends on order of collection
}

#[test]
fn u08_t02_crate_foo_imports_foo() {
    let source = r#"
        pub struct Foo;
        use crate::Foo;
    "#;
    let (def_map, _) = get_def_map(source);

    let root_mod = def_map.root;
    let foo_name = def_map.interner.intern("Foo");

    // Should be present (though potentially shadowed by the original,
    // but in our simple model, we just check if it resolves)
    let resolved = def_map.modules[root_mod].resolve(foo_name);
    assert!(resolved.is_some(), "Foo should be resolvable");
}

#[test]
fn u08_t03_self_bar_imports_bar() {
    let source = r#"
        pub struct Bar;
        use self::Bar;
    "#;
    let (def_map, _) = get_def_map(source);

    let root_mod = def_map.root;
    let bar_name = def_map.interner.intern("Bar");
    let resolved = def_map.modules[root_mod].resolve(bar_name);
    assert!(resolved.is_some(), "Bar should be resolvable");
}

#[test]
fn u08_t04_super_baz_imports_baz() {
    let source = r#"
        pub struct Baz;
        mod child {
            use super::Baz;
            // We verify by checking that 'Baz' resolves in child
            fn check() {
                // We can't run code, so we check the def map
            }
        }
    "#;
    let (def_map, _) = get_def_map(source);

    let root_mod = def_map.root;
    // Find child module
    let child_name = def_map.interner.intern("child");
    let child_id = def_map.modules[root_mod]
        .children
        .iter()
        .find(|(n, _)| *n == child_name)
        .map(|(_, id)| *id)
        .expect("Child module should exist");

    let baz_name = def_map.interner.intern("Baz");
    let resolved = def_map.modules[child_id].resolve(baz_name);
    assert!(resolved.is_some(), "Baz should be resolvable in child module via super");
}

#[test]
fn u08_t05_nested_imports() {
    let source = r#"
        mod std {
            pub mod io {
                pub struct Read;
                pub struct Write;
            }
        }
        use std::io::{Read, Write};
    "#;
    let (def_map, _) = get_def_map(source);

    let root_mod = def_map.root;
    let read_name = def_map.interner.intern("Read");
    let write_name = def_map.interner.intern("Write");

    assert!(def_map.modules[root_mod].resolve(read_name).is_some(), "Read should be imported");
    assert!(def_map.modules[root_mod].resolve(write_name).is_some(), "Write should be imported");
}

#[test]
fn u08_t06_glob_import() {
    let source = r#"
        mod std {
            pub mod io {
                pub struct Read;
                pub struct Write;
                // private struct Hidden;
            }
        }
        use std::io::*;
    "#;
    let (def_map, _) = get_def_map(source);

    let root_mod = def_map.root;
    let read_name = def_map.interner.intern("Read");
    let write_name = def_map.interner.intern("Write");

    assert!(def_map.modules[root_mod].resolve(read_name).is_some(), "Glob should import Read");
    assert!(def_map.modules[root_mod].resolve(write_name).is_some(), "Glob should import Write");
}
