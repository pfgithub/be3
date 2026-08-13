use super::*;

#[test]
fn read_destructure_unsupported_kind_errors() {
    let mut env = new_env();
    let src = vec![op_node(",", 0)];
    let mut targets = Vec::new();
    let Err(err) = read_destructure(&mut env, pos_at(0), &src, &mut targets) else {
        panic!("expected an error");
    };
    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };
    assert_eq!(
        e.entries[0].message,
        "Unsupported kind for destructuring: op"
    );
}
