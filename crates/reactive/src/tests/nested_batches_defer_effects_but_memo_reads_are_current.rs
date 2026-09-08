use super::*;

#[test]
fn nested_batches_defer_effects_but_memo_reads_are_current() {
    let scope = Scope::new();
    let (left, set_left) = create_signal(1);
    let (right, set_right) = create_signal(2);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sum = scope.run(|| {
        let sum = create_memo(move || left.get() + right.get());
        let observed = sum;
        let seen = seen.clone();
        create_effect(move || seen.borrow_mut().push(observed.get()));
        sum
    });
    batch(|| {
        set_left.set(10);
        assert_eq!(sum.get(), 12);
        batch(|| set_right.set(20));
        assert_eq!(*seen.borrow(), [3]);
        assert_eq!(sum.get(), 30);
    });
    assert_eq!(*seen.borrow(), [3, 30]);
}
