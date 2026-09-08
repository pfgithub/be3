use super::*;

#[test]
fn cleanup_reads_do_not_become_dependencies() {
    let scope = Scope::new();
    let (value, set_value) = create_signal(0);
    let (incidental, set_incidental) = create_signal(0);
    let calls = Rc::new(Cell::new(0));
    scope.run(|| {
        let calls = calls.clone();
        create_effect(move || {
            value.get();
            calls.set(calls.get() + 1);
            let incidental = incidental;
            on_cleanup(move || {
                incidental.get();
            });
        });
    });
    set_value.set(1);
    set_incidental.set(1);
    assert_eq!(calls.get(), 2);
}
