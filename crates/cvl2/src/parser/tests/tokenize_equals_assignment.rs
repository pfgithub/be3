use super::*;

#[test]
fn tokenize_equals_assignment() {
    let (result, _source) = tokenize_str("a=b");
    assert!(result.errors.is_empty());
    assert_eq!(result.result.len(), 1);

    let SyntaxNode::BinaryExpression(binary) = &result.result[0] else {
        panic!("expected a binary expression, got {:?}", result.result[0]);
    };
    assert_eq!(binary.tag, OpTag::Assign);
    assert_eq!(binary.items.len(), 3);
    assert!(matches!(&binary.items[1], SyntaxNode::Operator(o) if o.op == "="));
}
