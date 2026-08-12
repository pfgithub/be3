use super::*;

#[test]
fn read_destructure_extra_item_reports_error() {
    let mut env = new_env();
    let src = vec![normal_ident("a", 0), normal_ident("b", 1)];
    let Err(err) = read_destructure(&mut env, pos_at(0), &src) else {
        panic!("expected an error");
    };

    assert!(err.e.entries[0]
        .message
        .starts_with("Unexpected item for destructuring."));
}
