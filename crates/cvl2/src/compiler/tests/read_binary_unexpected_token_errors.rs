use super::*;

#[test]
fn read_binary_unexpected_token_errors() {
    let mut env = new_env();
    let src = vec![binary_node(OpTag::Sep, vec![normal_ident("a", 0)], 0)];
    let err = read_binary(&mut env, pos_at(0), &src, OpTag::Sep).unwrap_err();
    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };
    assert_eq!(e.entries[0].message, "Unexpected token in sep: ident");
}
