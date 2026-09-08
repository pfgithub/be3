use super::*;

#[test]
fn signals_track_changes_and_skip_equal_sets() {
    let scope = Scope::new();
    let (value, set_value) = create_signal(1);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || seen.borrow_mut().push(value.get()));
    });
    set_value.set(1);
    set_value.set(2);
    set_value.update(|value| *value += 1);
    assert_eq!(*seen.borrow(), [1, 2, 3]);
    assert_eq!(value.get_untracked(), 3);
}
