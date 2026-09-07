use super::*;

#[test]
fn runaway_effects_are_stopped_and_scheduler_remains_usable() {
    let scope = Scope::new();
    let (value, set_value) = create_signal(0);
    let effect = RefCell::new(None);
    assert!(catch_unwind(AssertUnwindSafe(|| scope.run(|| {
        effect.replace(Some(create_effect(move || set_value.set(value.get() + 1))));
    })))
    .is_err());
    assert!(effect.borrow().as_ref().unwrap().is_disposed());
    let calls = Rc::new(Cell::new(0));
    scope.run(|| {
        let calls = calls.clone();
        create_effect(move || calls.set(calls.get() + 1));
    });
    assert_eq!(calls.get(), 1);
}
