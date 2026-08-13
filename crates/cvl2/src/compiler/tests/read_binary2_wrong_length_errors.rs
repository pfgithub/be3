use super::*;

#[test]
fn read_binary2_wrong_length_errors() {
    let mut env = new_env();
    let src = vec![binary_node(
        OpTag::Def,
        vec![op_seg(vec![normal_ident("a", 0)], 0)],
        0,
    )];
    let err = read_binary2(&mut env, &src, OpTag::Def).unwrap_err();
    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };
    assert_eq!(e.entries[0].message, "Expected LHS op RHS, found not that");
}
