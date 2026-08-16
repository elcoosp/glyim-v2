//! Tests for the `LangItems` registry (de-stubbing plan §1.1).

use crate::lang_items::{LangItem, LangItemError, LangItems};
use glyim_core::def_id::{CrateId, DefId, LocalDefId};

fn def(krate: u32, local: u32) -> DefId {
    DefId::new(CrateId::from_raw(krate), LocalDefId::from_raw(local))
}

#[test]
fn register_then_require_succeeds() {
    let mut items = LangItems::default();
    let d = def(0, 1000);
    items.register(LangItem::Range, d).expect("first registration");
    assert_eq!(items.get(LangItem::Range), Some(d));
    assert_eq!(items.require(LangItem::Range).unwrap(), d);
}

#[test]
fn require_missing_is_error() {
    let items = LangItems::default();
    match items.require(LangItem::Option) {
        Err(LangItemError::Missing(LangItem::Option)) => {}
        other => panic!("expected Missing(Option), got {:?}", other),
    }
    assert_eq!(items.get(LangItem::Option), None);
}

#[test]
fn duplicate_distinct_def_id_errors() {
    let mut items = LangItems::default();
    items.register(LangItem::Vec, def(0, 1)).unwrap();
    let err = items.register(LangItem::Vec, def(0, 2)).unwrap_err();
    match err {
        LangItemError::Duplicate {
            item: LangItem::Vec,
            existing,
            new,
        } => {
            assert_eq!(existing, def(0, 1));
            assert_eq!(new, def(0, 2));
        }
        other => panic!("expected Duplicate, got {:?}", other),
    }
}

#[test]
fn re_register_same_def_id_is_idempotent() {
    let mut items = LangItems::default();
    let d = def(1, 42);
    items.register(LangItem::Box, d).unwrap();
    // Re-registering the identical DefId must succeed (no spurious Duplicate).
    items.register(LangItem::Box, d).expect("idempotent re-registration");
    assert_eq!(items.require(LangItem::Box).unwrap(), d);
}
