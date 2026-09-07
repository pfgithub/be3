use super::*;

#[test]
fn a_panicking_effect_can_run_again() {
    let scope = Scope::new();
    let (value, set_value) = create_signal(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || {
            let value = value.get();
            assert_ne!(value, 1);
            seen.borrow_mut().push(value);
        });
    });
    assert!(catch_unwind(AssertUnwindSafe(|| set_value.set(1))).is_err());
    set_value.set(2);
    assert_eq!(*seen.borrow(), [0, 2]);
}
