use super::*;

#[test]
fn a_selector_memo_tracks_one_key_without_watching_the_source() {
    let scope = Scope::new();
    let (selected, set_selected) = create_signal(0usize);
    let calls = Rc::new(Cell::new(0));
    let (selector, first) = scope.run(|| {
        let selector = create_selector(move || selected.get());
        let first = selector.memo(0);
        (selector, first)
    });
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run({
        let calls = calls.clone();
        let seen = seen.clone();
        let first = first.clone();
        move || {
            create_effect(move || {
                calls.set(calls.get() + 1);
                seen.borrow_mut().push(first.get());
            })
        }
    });
    assert_eq!(*seen.borrow(), [true]);
    set_selected.set(1);
    assert_eq!(*seen.borrow(), [true, false]);
    set_selected.set(2);
    set_selected.set(3);
    assert_eq!(*seen.borrow(), [true, false]);
    assert_eq!(calls.get(), 2);
    assert_eq!(selector.watched_keys(), 1);
    set_selected.set(0);
    assert_eq!(*seen.borrow(), [true, false, true]);
}
