use super::*;

#[test]
fn nested_effects_are_disposed_before_parent_reruns() {
    let scope = Scope::new();
    let (parent, set_parent) = create_signal(1);
    let (child, set_child) = create_signal(10);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || {
            let parent = parent.get();
            let child = child;
            let seen = seen.clone();
            create_effect(move || seen.borrow_mut().push((parent, child.get())));
        });
    });
    batch(|| {
        set_child.set(20);
        set_parent.set(2);
    });
    set_child.set(30);
    assert_eq!(*seen.borrow(), [(1, 10), (2, 20), (2, 30)]);
}
