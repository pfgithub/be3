use super::*;

#[test]
fn scopes_dispose_effects_and_run_cleanup_once() {
    let parent = Scope::new();
    let child = parent.run(Scope::new);
    let (value, set_value) = create_signal(1);
    let calls = Rc::new(Cell::new(0));
    let cleanups = Rc::new(Cell::new(0));
    let effect = child.run(|| {
        let calls = calls.clone();
        let cleanups = cleanups.clone();
        create_effect(move || {
            value.get();
            calls.set(calls.get() + 1);
            let cleanups = cleanups.clone();
            on_cleanup(move || cleanups.set(cleanups.get() + 1));
        })
    });
    set_value.set(2);
    assert_eq!(cleanups.get(), 1);
    parent.dispose();
    assert!(child.is_disposed());
    assert!(effect.is_disposed());
    child.dispose();
    set_value.set(3);
    assert_eq!(calls.get(), 2);
    assert_eq!(cleanups.get(), 2);
}
