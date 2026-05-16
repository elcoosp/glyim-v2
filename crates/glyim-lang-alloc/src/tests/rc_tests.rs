use crate::Rc;

#[test]
fn rc_cloning_and_ref_count() {
    let rc1 = Rc::new(10);
    assert_eq!(Rc::strong_count(&rc1), 1);
    {
        let rc2 = rc1.clone();
        assert_eq!(Rc::strong_count(&rc1), 2);
        assert_eq!(*rc2, 10);
    }
    assert_eq!(Rc::strong_count(&rc1), 1);
}

#[test]
fn rc_multiple_clones() {
    let rc1 = Rc::new(42);
    let rc2 = rc1.clone();
    let rc3 = rc1.clone();
    let rc4 = rc2.clone();
    assert_eq!(Rc::strong_count(&rc1), 4);
    assert_eq!(*rc1, 42);
    assert_eq!(*rc2, 42);
    assert_eq!(*rc3, 42);
    assert_eq!(*rc4, 42);
    drop(rc2);
    drop(rc3);
    assert_eq!(Rc::strong_count(&rc1), 2);
    drop(rc4);
    assert_eq!(Rc::strong_count(&rc1), 1);
}

#[test]
fn rc_deref_works() {
    let rc = Rc::new(100);
    assert_eq!(*rc, 100);
}

#[test]
fn rc_clone_drop_repeated() {
    let rc = Rc::new(1);
    for _ in 0..100 {
        let clone = rc.clone();
        assert_eq!(*clone, 1);
    }
    assert_eq!(Rc::strong_count(&rc), 1);
}

#[test]
fn rc_with_zero() {
    let rc = Rc::new(0usize);
    assert_eq!(*rc, 0);
    let clone = rc.clone();
    assert_eq!(*clone, 0);
    assert_eq!(Rc::strong_count(&rc), 2);
}
