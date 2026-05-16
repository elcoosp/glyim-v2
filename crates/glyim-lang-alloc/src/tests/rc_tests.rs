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
