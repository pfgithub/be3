use super::*;

#[test]
fn untracked_memo_reads_still_refresh_without_subscribing() {
    let scope = Scope::new();
    let (value, set_value) = create_signal(1);
    let (trigger, set_trigger) = create_signal(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let memo = create_memo(move || value.get() * 2);
        let seen = seen.clone();
        create_effect(move || {
            trigger.get();
            seen.borrow_mut().push(memo.get_untracked());
        });
    });
    set_value.set(2);
    set_trigger.set(1);
    assert_eq!(*seen.borrow(), [2, 4]);
}
