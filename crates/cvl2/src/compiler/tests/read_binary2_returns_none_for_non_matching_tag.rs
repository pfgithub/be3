use super::*;

#[test]
fn read_binary2_returns_none_for_non_matching_tag() {
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
    let result = read_binary2(&mut env, &src, OpTag::Def).unwrap();
    assert!(result.is_none());
}
