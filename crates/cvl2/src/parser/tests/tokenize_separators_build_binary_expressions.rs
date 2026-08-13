use super::*;

fn assert_two_item_separator(src: &str, expected_op: &str) {
    let (result, _source) = tokenize_str(src);
    assert!(
        result.errors.is_empty(),
        "unexpected errors for {src:?}: {:?}",
        result.errors
    );
    assert_eq!(result.result.len(), 1);

    let SyntaxNode::BinaryExpression(binary) = &result.result[0] else {
        panic!(
            "expected a binary expression for {src:?}, got {:?}",
            result.result[0]
        );
    };
    assert_eq!(binary.tag, OpTag::Sep);
    assert_eq!(binary.items.len(), 3);

    match &binary.items[0] {
        SyntaxNode::OperatorSegment(seg) => {
            assert!(matches!(&seg.items[..], [SyntaxNode::Identifier(t)] if t.str == "a"));
        }
        other => panic!("expected opSeg, got {other:?}"),
    }
    assert!(
        matches!(&binary.items[1], SyntaxNode::Operator(o) if o.op == expected_op && o.op_tag == OpTag::Sep)
    );
    match &binary.items[2] {
        SyntaxNode::OperatorSegment(seg) => {
            assert!(matches!(&seg.items[..], [SyntaxNode::Identifier(t)] if t.str == "b"));
        }
        other => panic!("expected opSeg, got {other:?}"),
    }
}

#[test]
fn tokenize_separators_build_binary_expressions() {
    assert_two_item_separator("a,b", ",");
    assert_two_item_separator("a;b", ";");
    assert_two_item_separator("a\nb", "\n");
}
