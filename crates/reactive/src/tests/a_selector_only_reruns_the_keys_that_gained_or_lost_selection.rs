use super::*;

#[test]
fn a_selector_only_reruns_the_keys_that_gained_or_lost_selection() {
    let scope = Scope::new();
    let (selected, set_selected) = create_signal(Some(0usize));
    let runs = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let selector = create_selector(move || selected.get());
        for index in 0..4 {
            let selector = selector.clone();
            let runs = runs.clone();
            create_effect(move || {
                let selected = selector.is_selected(&Some(index));
                runs.borrow_mut().push((index, selected));
            });
        }
    });
    assert_eq!(
        *runs.borrow(),
        [(0, true), (1, false), (2, false), (3, false)]
    );
    runs.borrow_mut().clear();
    set_selected.set(Some(2));
    assert_eq!(*runs.borrow(), [(0, false), (2, true)]);
    runs.borrow_mut().clear();
    set_selected.set(None);
    assert_eq!(*runs.borrow(), [(2, false)]);
    runs.borrow_mut().clear();
    set_selected.set(Some(3));
    assert_eq!(*runs.borrow(), [(3, true)]);
}
