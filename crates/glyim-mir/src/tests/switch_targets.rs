use crate::*;

#[test]
fn switch_targets_new_empty() {
    let targets = SwitchTargets::new(Box::new([]), BasicBlockIdx::from_raw(0));
    assert_eq!(targets.otherwise(), BasicBlockIdx::from_raw(0));
    assert_eq!(targets.iter().count(), 0);
}

#[test]
fn switch_targets_new_with_branches() {
    let targets = SwitchTargets::new(
        Box::new([
            (0u128, BasicBlockIdx::from_raw(1)),
            (1u128, BasicBlockIdx::from_raw(2)),
            (5u128, BasicBlockIdx::from_raw(3)),
        ]),
        BasicBlockIdx::from_raw(4),
    );
    assert_eq!(targets.otherwise(), BasicBlockIdx::from_raw(4));
    let branches: Vec<_> = targets.iter().collect();
    assert_eq!(branches.len(), 3);
    assert_eq!(branches[0], (0u128, BasicBlockIdx::from_raw(1)));
    assert_eq!(branches[1], (1u128, BasicBlockIdx::from_raw(2)));
    assert_eq!(branches[2], (5u128, BasicBlockIdx::from_raw(3)));
}

#[test]
fn switch_targets_if_switch() {
    let then_bb = BasicBlockIdx::from_raw(10);
    let else_bb = BasicBlockIdx::from_raw(20);
    let targets = SwitchTargets::if_switch(then_bb, else_bb);

    assert_eq!(targets.otherwise(), else_bb);
    let branches: Vec<_> = targets.iter().collect();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0], (1u128, then_bb));
}

#[test]
fn switch_targets_iter_is_cow() {
    let targets = SwitchTargets::new(
        Box::new([(42u128, BasicBlockIdx::from_raw(7))]),
        BasicBlockIdx::from_raw(8),
    );
    for (val, bb) in targets.iter() {
        assert_eq!(val, 42u128);
        assert_eq!(bb, BasicBlockIdx::from_raw(7));
    }
}
