use super::*;

#[test]
fn an_effect_with_cleanup_is_kept_without_dependencies() {
    let scope = Scope::new();
    let cleanups = Rc::new(Cell::new(0));
    let effect = scope.run({
        let cleanups = cleanups.clone();
        || {
            create_effect(move || {
                let cleanups = cleanups.clone();
                on_cleanup(move || cleanups.set(cleanups.get() + 1));
            })
        }
    });
    assert!(!effect.is_disposed());
    assert_eq!(cleanups.get(), 0);
    scope.dispose();
    assert!(effect.is_disposed());
    assert_eq!(cleanups.get(), 1);
}
