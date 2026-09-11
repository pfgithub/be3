use super::*;

#[test]
fn selector_reads_are_current_inside_a_batch() {
    let scope = Scope::new();
    let (selected, set_selected) = create_signal(0usize);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let selector = scope.run(|| create_selector(move || selected.get()));
    scope.run({
        let selector = selector.clone();
        let seen = seen.clone();
        move || create_effect(move || seen.borrow_mut().push(selector.is_selected(&2)))
    });
    assert_eq!(*seen.borrow(), [false]);
    batch(|| {
        set_selected.set(1);
        assert!(!selector.is_selected(&2));
        set_selected.set(2);
        assert!(selector.is_selected(&2));
        assert_eq!(seen.borrow().len(), 1);
    });
    assert_eq!(*seen.borrow(), [false, true]);
}
