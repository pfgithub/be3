use super::*;

#[test]
fn memo_panic_can_be_retried() {
    let scope = Scope::new();
    let (value, set_value) = create_signal(0);
    let memo = scope.run(|| {
        create_memo(move || {
            let value = value.get();
            assert_ne!(value, 1);
            value * 2
        })
    });
    set_value.set(1);
    assert!(catch_unwind(AssertUnwindSafe(|| memo.get())).is_err());
    set_value.set(2);
    assert_eq!(memo.get(), 4);
}
