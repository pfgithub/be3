use super::*;

#[test]
fn effects_can_write_signals_and_settle_without_recursion() {
    let scope = Scope::new();
    let (value, set_value) = create_signal(0);
    let observed = value.clone();
    scope.run(|| {
        create_effect(move || {
            let value = observed.get();
            if value < 5 {
                set_value.set(value + 1);
            }
        });
    });
    assert_eq!(value.get(), 5);
}
