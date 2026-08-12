use super::*;

#[test]
fn tokenize_mixing_bind_operators_reports_error() {
    let (result, _source) = tokenize_str("a::b.=c");

    assert_eq!(result.errors.len(), 1);
    assert_eq!(
        result.errors[0].entries[0].message,
        "mixing operators disallowed"
    );

    assert_eq!(result.result.len(), 1);
    let SyntaxNode::BinaryExpression(binary) = &result.result[0] else {
        panic!("expected a binary expression, got {:?}", result.result[0]);
    };
    assert_eq!(binary.tag, OpTag::Def);
    assert_eq!(binary.items.len(), 5);
    assert!(matches!(&binary.items[1], SyntaxNode::Operator(o) if o.op == "::"));
    assert!(matches!(&binary.items[3], SyntaxNode::Operator(o) if o.op == ".="));
}
