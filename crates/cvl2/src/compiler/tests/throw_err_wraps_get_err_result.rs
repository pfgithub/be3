use super::*;

#[test]
fn throw_err_wraps_get_err_result() {
    let env = new_env();
    let err = throw_err(&env, Some(pos_at(2)), "bad thing", None);
    assert_eq!(err.e.entries[0].message, "bad thing");
    assert_eq!(err.e.entries[0].pos, Some(pos_at(2)));
    assert!(err.to_string().contains("bad thing"));
}
