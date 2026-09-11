use super::*;

#[test]
fn a_selector_forgets_keys_whose_watchers_were_disposed() {
    let scope = Scope::new();
    let (selected, set_selected) = create_signal(0usize);
    let selector = scope.run(|| create_selector(move || selected.get()));
    let (watch_left, set_side) = create_signal(true);
    let rows = scope.run(Scope::new);
    rows.run({
        let selector = selector.clone();
        move || {
            for index in 0..3 {
                let selector = selector.clone();
                create_effect(move || {
                    selector.is_selected(&index);
                });
            }
            create_effect(move || {
                selector.is_selected(&if watch_left.get() { 0 } else { 9 });
            });
        }
    });
    assert_eq!(selector.watched_keys(), 3);
    set_side.set(false);
    assert_eq!(selector.watched_keys(), 4);
    set_side.set(true);
    assert_eq!(selector.watched_keys(), 3);
    rows.dispose();
    assert_eq!(selector.watched_keys(), 0);
    set_selected.set(1);
    assert_eq!(selector.watched_keys(), 0);
}
