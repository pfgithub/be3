use super::*;

#[test]
fn throw_err_wraps_get_err_result() {
    let env = new_env();
    let err = throw_err(&env, Some(pos_at(2)), "bad thing", None, None);
    let PositionedError::Fresh(e) = &err else {
        panic!("expected a fresh error");
    };
    assert_eq!(e.entries[0].message, "bad thing");
    assert_eq!(e.entries[0].pos, Some(pos_at(2)));
    assert!(err.to_string().contains("bad thing"));
}
