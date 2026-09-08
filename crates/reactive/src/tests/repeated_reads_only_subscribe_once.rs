use super::*;

#[test]
fn repeated_reads_only_subscribe_once() {
    let scope = Scope::new();
    let (value, set_value) = create_signal(1);
    let observed = value;
    scope.run(|| {
        create_effect(move || {
            observed.get();
            observed.get();
        });
    });
    for next in 2..10 {
        set_value.set(next);
        assert_eq!(value.inner().source.subscribers.borrow().len(), 1);
    }
    scope.dispose();
    assert!(value.inner().source.subscribers.borrow().is_empty());
}
