use super::*;

#[test]
fn analyze_namespace_errors_on_non_key_bind_target() {
    let mut env = new_env();
    let src = vec![pub_binding(
        builtin_ident("builtin", 0),
        normal_ident("x", 2),
        0,
    )];

    let result = analyze_namespace(&mut env, pos_at(0), &src);
    let Err(err) = result else {
        panic!("expected an error");
    };
    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };
    assert_eq!(
        e.entries[0].message,
        "Expected value of type Key, got Namespace"
    );
}
