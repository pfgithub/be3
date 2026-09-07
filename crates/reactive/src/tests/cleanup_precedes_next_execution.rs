use super::*;

#[test]
fn cleanup_precedes_next_execution() {
    let scope = Scope::new();
    let (value, set_value) = create_signal(0);
    let events = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let events = events.clone();
        create_effect(move || {
            let value = value.get();
            events.borrow_mut().push(("run", value));
            let events = events.clone();
            on_cleanup(move || events.borrow_mut().push(("cleanup", value)));
        });
    });
    set_value.set(1);
    drop(scope);
    assert_eq!(
        *events.borrow(),
        [("run", 0), ("cleanup", 0), ("run", 1), ("cleanup", 1)]
    );
}
