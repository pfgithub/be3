use super::*;

#[test]
fn clone_brings_cloned_handles_into_the_closure() {
    let scope = Scope::new();
    let (value, set_value) = create_signal(1);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let doubled = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        create_effect(clone!(seen value -> move || seen.borrow_mut().push(value.get())));
        create_effect(clone!(seen doubled -> move || {
            doubled.borrow_mut().push(seen.borrow().len() * 2);
            value.get();
        }));
    });
    set_value.set(2);
    assert_eq!(*seen.borrow(), vec![1, 2]);
    assert_eq!(*doubled.borrow(), vec![2, 4]);
}
