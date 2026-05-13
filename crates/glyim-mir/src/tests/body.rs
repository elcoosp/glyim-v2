use crate::*;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};

#[test]
fn dummy_creates_valid_structure() {
    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0));
    let body = Body::dummy(owner);

    assert_eq!(body.owner.krate, CrateId::from_raw(0));
    assert_eq!(body.owner.local_id, LocalDefId::from_raw(0));
    assert_eq!(body.arg_count, 0);
    assert_eq!(body.return_ty, Ty::ERROR);
    assert_eq!(body.basic_blocks.len(), 1);
    assert_eq!(body.locals.len(), 1);
    assert_eq!(body.locals[LocalIdx::from_raw(0)].ty, Ty::ERROR);
    assert!(body.var_debug_info.is_empty());
}
