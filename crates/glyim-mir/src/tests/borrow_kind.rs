use crate::*;

#[test]
fn borrow_kind_shared() {
    let kind = BorrowKind::Shared;
    assert_eq!(kind, BorrowKind::Shared);
}

#[test]
fn borrow_kind_unique() {
    let kind = BorrowKind::Unique;
    assert_eq!(kind, BorrowKind::Unique);
}

#[test]
fn borrow_kind_mut_no_two_phase() {
    let kind = BorrowKind::Mut {
        allow_two_phase_borrow: false,
    };
    assert_eq!(
        kind,
        BorrowKind::Mut {
            allow_two_phase_borrow: false
        }
    );
}

#[test]
fn borrow_kind_mut_two_phase() {
    let kind = BorrowKind::Mut {
        allow_two_phase_borrow: true,
    };
    assert_eq!(
        kind,
        BorrowKind::Mut {
            allow_two_phase_borrow: true
        }
    );
}
