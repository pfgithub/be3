use super::*;

#[test]
fn trim_ws_filters_whitespace_nodes() {
    let src = vec![
        normal_ident("a", 0),
        ws_node(1),
        normal_ident("b", 2),
        ws_node(3),
    ];
    let result = trim_ws(&src);
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], SyntaxNode::Identifier(id) if id.str == "a"));
    assert!(matches!(&result[1], SyntaxNode::Identifier(id) if id.str == "b"));
}
