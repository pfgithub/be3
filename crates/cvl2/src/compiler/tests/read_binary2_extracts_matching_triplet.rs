use super::*;

#[test]
fn read_binary2_extracts_matching_triplet() {
    let mut env = new_env();
    let src = vec![binary_node(
        OpTag::Def,
        vec![
            op_seg(vec![normal_ident("a", 0)], 0),
            op_node("::", 1),
            op_seg(vec![normal_ident("b", 2)], 2),
        ],
        0,
    )];
    let (lhs, op, rhs) = read_binary2(&mut env, &src, OpTag::Def).unwrap().unwrap();
    assert!(matches!(&lhs.items[0], SyntaxNode::Identifier(id) if id.str == "a"));
    assert_eq!(op.op, "::");
    assert!(matches!(&rhs.items[0], SyntaxNode::Identifier(id) if id.str == "b"));
}
