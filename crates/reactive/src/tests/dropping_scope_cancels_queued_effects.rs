use super::*;

#[test]
fn dropping_scope_cancels_queued_effects() {
    let calls = Rc::new(Cell::new(0));
    batch(|| {
        let scope = Scope::new();
        scope.run(|| {
            let calls = calls.clone();
            create_effect(move || calls.set(calls.get() + 1));
        });
    });
    assert_eq!(calls.get(), 0);
}
