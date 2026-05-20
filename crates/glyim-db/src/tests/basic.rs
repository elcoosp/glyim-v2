use std::sync::Arc;

use crate::{CrateConfig, Database};

#[test]
fn test_database_tyctx_roundtrip() {
    // S23-T01: Database stores and retrieves TyCtx correctly
    let db = Database::new(CrateConfig {
        name: "test".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        opt_level: 0,
    });
    // Create a TyCtx (frozen) from a mutable builder
    let ctx_mut = glyim_test::test_ty_ctx();
    let bool_ty = ctx_mut.bool_ty();
    let ctx = ctx_mut.freeze();
    // Store it (move, no clone)
    db.set_ty_ctx(ctx);
    // Retrieve and verify
    let guard = db.ty_ctx();
    assert!(guard.is_some());
    let retrieved = guard.as_ref().unwrap();
    // Compare a simple property: bool_ty should be Ty::BOOL
    assert_eq!(retrieved.bool_ty(), bool_ty);
}

#[test]
fn test_database_interner_mut() {
    // S23-T02: Database stores and retrieves Interner mutably
    let mut db = Database::new(CrateConfig {
        name: "test".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        opt_level: 0,
    });
    let name = db.intern_mut().intern("hello");
    // Retrieve via shared reference
    assert_eq!(db.interner().resolve(name), "hello");
}

#[test]
fn test_database_vfs_content() {
    // S23-T03: Database Vfs returns correct file contents
    let db = Database::new(CrateConfig {
        name: "test".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        opt_level: 0,
    });
    let content: Arc<str> = Arc::from("fn main() {}");
    let file_id = db
        .vfs()
        .add_file_content(&std::path::PathBuf::from("main.g"), content.clone());
    let retrieved = db.vfs().file_content(file_id).unwrap();
    assert_eq!(&*retrieved, "fn main() {}");
}
