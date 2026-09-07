use super::*;

#[test]
fn conditional_dependencies_are_replaced() {
    let scope = Scope::new();
    let (choose_left, choose) = create_signal(true);
    let (left, set_left) = create_signal(1);
    let (right, set_right) = create_signal(10);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || {
            seen.borrow_mut().push(if choose_left.get() {
                left.get()
            } else {
                right.get()
            })
        });
    });
    set_right.set(11);
    choose.set(false);
    set_left.set(2);
    set_right.set(12);
    assert_eq!(*seen.borrow(), [1, 11, 12]);
}
