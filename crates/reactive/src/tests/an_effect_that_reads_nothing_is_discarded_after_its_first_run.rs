use super::*;

#[test]
fn an_effect_that_reads_nothing_is_discarded_after_its_first_run() {
    let scope = Scope::new();
    let calls = Rc::new(Cell::new(0));
    let resource = Rc::new(());
    let weak = Rc::downgrade(&resource);
    let effect = scope.run({
        let calls = calls.clone();
        || {
            create_effect(move || {
                let _ = &resource;
                calls.set(calls.get() + 1);
            })
        }
    });
    assert_eq!(calls.get(), 1);
    assert!(effect.is_disposed());
    assert!(weak.upgrade().is_none());
}
