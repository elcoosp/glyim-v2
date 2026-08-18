//! Plan §22.6: unit tests for the def-map `insert_use` auto-import helper.

use crate::insert_use;

#[test]
fn inserts_new_use_at_top_of_empty_file() {
    let src = "fn main() {}\n";
    let out = insert_use(src, "std::collections::HashMap");
    assert_eq!(out, "use std::collections::HashMap;\nfn main() {}\n");
}

#[test]
fn appends_to_existing_use_block() {
    let src = "use std::fmt;\n\nfn main() {}\n";
    let out = insert_use(src, "std::collections::HashMap");
    assert_eq!(
        out,
        "use std::fmt;\nuse std::collections::HashMap;\n\nfn main() {}\n"
    );
}

#[test]
fn is_idempotent_for_already_imported_path() {
    let src = "use std::collections::HashMap;\nfn main() {}\n";
    let out = insert_use(src, "std::collections::HashMap");
    assert_eq!(out, src);
}

#[test]
fn skips_shebang_before_inserting() {
    let src = "#!/usr/bin/env glyim\nfn main() {}\n";
    let out = insert_use(src, "foo::bar");
    assert_eq!(out, "#!/usr/bin/env glyim\nuse foo::bar;\nfn main() {}\n");
}

#[test]
fn does_not_treat_indented_use_as_top_level() {
    // A `use` inside a function body must not be treated as the import block.
    let src = "fn f() {\n    use inner::thing;\n}\n";
    let out = insert_use(src, "outer::other");
    assert_eq!(
        out,
        "use outer::other;\nfn f() {\n    use inner::thing;\n}\n"
    );
}

#[test]
fn group_head_dedup_detects_existing_group() {
    // The exact same group path is recognized as already imported.
    let src = "use std::collections::{HashMap, HashSet};\nfn main() {}\n";
    let out = insert_use(src, "std::collections::{HashMap, HashSet}");
    assert_eq!(out, src);
}
