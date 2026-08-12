use super::*;

#[test]
fn add_err_pushes_error_into_env() {
    let mut env = new_env();
    assert!(env.errors.is_empty());
    add_err(&mut env, Some(pos_at(1)), "oops", None);
    assert_eq!(env.errors.len(), 1);
    assert_eq!(env.errors[0].entries[0].message, "oops");
}
