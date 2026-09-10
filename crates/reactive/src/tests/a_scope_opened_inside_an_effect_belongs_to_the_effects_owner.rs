use super::*;

#[test]
fn a_scope_opened_inside_an_effect_belongs_to_the_effects_owner() {
    let outer = Scope::new();
    let (tick, set_tick) = create_signal(0);
    let cleanups = Rc::new(Cell::new(0));
    let opened: Rc<RefCell<Vec<Scope>>> = Rc::new(RefCell::new(Vec::new()));

    let sink = cleanups.clone();
    let kept = opened.clone();
    outer.run(move || {
        create_effect(move || {
            tick.get();
            let scope = owner_scope()
                .expect("an effect always has an owner")
                .child()
                .expect("the owner is alive while the effect runs");
            let sink = sink.clone();
            scope.run(move || on_cleanup(move || sink.set(sink.get() + 1)));
            kept.borrow_mut().push(scope);
        });
    });

    set_tick.set(1);
    assert_eq!(opened.borrow().len(), 2, "the effect ran twice");
    assert_eq!(
        cleanups.get(),
        0,
        "a scope opened inside an effect must survive that effect re-running"
    );

    outer.dispose();
    assert_eq!(
        cleanups.get(),
        2,
        "disposing the owner must dispose every scope opened inside its effects"
    );
}
