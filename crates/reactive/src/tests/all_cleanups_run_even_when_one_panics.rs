use super::*;

#[test]
fn all_cleanups_run_even_when_one_panics() {
    let scope = Scope::new();
    let calls = Rc::new(Cell::new(0));
    scope.run(|| {
        let calls = calls.clone();
        on_cleanup(move || calls.set(calls.get() + 1));
        on_cleanup(|| panic!("cleanup failed"));
    });
    assert!(catch_unwind(AssertUnwindSafe(|| scope.dispose())).is_err());
    assert_eq!(calls.get(), 1);
    assert!(scope.is_disposed());
}
