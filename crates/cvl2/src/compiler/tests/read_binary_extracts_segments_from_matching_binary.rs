use super::*;

#[test]
fn read_binary_extracts_segments_from_matching_binary() {
    let mut env = new_env();
    let src = vec![binary_node(
        OpTag::Sep,
        vec![
            op_seg(vec![normal_ident("a", 0)], 0),
            op_node(",", 1),
            op_seg(vec![normal_ident("b", 2)], 2),
        ],
        0,
    )];
    let result = read_binary(&mut env, pos_at(0), &src, OpTag::Sep).unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0].items[0], SyntaxNode::Identifier(id) if id.str == "a"));
    assert!(matches!(&result[1].items[0], SyntaxNode::Identifier(id) if id.str == "b"));
}
