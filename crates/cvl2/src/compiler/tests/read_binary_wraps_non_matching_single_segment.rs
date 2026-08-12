use super::*;

#[test]
fn read_binary_wraps_non_matching_single_segment() {
    let mut env = new_env();
    let src = vec![normal_ident("a", 0)];
    let result = read_binary(&mut env, pos_at(0), &src, OpTag::Sep).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].items.len(), 1);
    assert!(matches!(&result[0].items[0], SyntaxNode::Identifier(id) if id.str == "a"));
}
