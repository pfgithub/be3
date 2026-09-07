use super::*;

#[test]
fn panic_restores_tracking_and_batching() {
    let scope = Scope::new();
    let (value, set_value) = create_signal(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || seen.borrow_mut().push(value.get()));
    });
    assert!(catch_unwind(AssertUnwindSafe(|| batch(|| {
        set_value.set(1);
        untrack(|| panic!("abort batch"));
    })))
    .is_err());
    set_value.set(2);
    assert_eq!(*seen.borrow(), [0, 2]);
}
