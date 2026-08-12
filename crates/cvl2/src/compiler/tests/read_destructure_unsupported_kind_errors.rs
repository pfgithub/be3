use super::*;

#[test]
fn read_destructure_unsupported_kind_errors() {
    let mut env = new_env();
    let src = vec![op_node(",", 0)];
    let Err(err) = read_destructure(&mut env, pos_at(0), &src) else {
        panic!("expected an error");
    };
    assert_eq!(
        err.e.entries[0].message,
        "Unsupported kind for destructuring: op"
    );
}
