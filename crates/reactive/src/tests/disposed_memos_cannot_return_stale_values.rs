use super::*;

#[test]
fn disposed_memos_cannot_return_stale_values() {
    let scope = Scope::new();
    let memo = scope.run(|| create_memo(|| 1));
    scope.dispose();
    assert!(catch_unwind(AssertUnwindSafe(|| memo.get())).is_err());
}
