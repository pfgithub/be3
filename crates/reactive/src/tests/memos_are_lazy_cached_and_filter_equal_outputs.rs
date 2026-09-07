use super::*;

#[test]
fn memos_are_lazy_cached_and_filter_equal_outputs() {
    let scope = Scope::new();
    let (value, set_value) = create_signal(1);
    let calls = Rc::new(Cell::new(0));
    let memo = scope.run(|| {
        let calls = calls.clone();
        create_memo(move || {
            calls.set(calls.get() + 1);
            value.get() % 2
        })
    });
    set_value.set(3);
    assert_eq!(calls.get(), 1);
    assert_eq!(memo.get(), 1);
    assert_eq!(memo.get(), 1);
    assert_eq!(calls.get(), 2);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || seen.borrow_mut().push(memo.get()));
    });
    set_value.set(5);
    set_value.set(6);
    assert_eq!(*seen.borrow(), [1, 0]);
}
