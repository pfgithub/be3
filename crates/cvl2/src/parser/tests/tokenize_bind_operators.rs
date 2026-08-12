use super::*;

#[test]
fn tokenize_bind_operators() {
    for (op, tag) in [("::", OpTag::Def), (".=", OpTag::Pub), (":=", OpTag::Var)] {
        let src = format!("a{op}b");
        let (result, _source) = tokenize_str(&src);
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
        assert_eq!(binary.tag, tag);
        assert_eq!(binary.items.len(), 3);
        assert!(matches!(&binary.items[1], SyntaxNode::Operator(o) if o.op == op));
    }
}
