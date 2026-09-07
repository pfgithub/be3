use super::*;

#[test]
fn untracked_reads_do_not_subscribe() {
    let scope = Scope::new();
    let (trigger, set_trigger) = create_signal(0);
    let (incidental, set_incidental) = create_signal(1);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || {
            trigger.get();
            seen.borrow_mut()
                .push(untrack(|| incidental.get()) + incidental.get_untracked());
        });
    });
    set_incidental.set(2);
    set_trigger.set(1);
    assert_eq!(*seen.borrow(), [2, 4]);
}
