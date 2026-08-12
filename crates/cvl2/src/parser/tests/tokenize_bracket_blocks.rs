use super::*;

#[test]
fn tokenize_bracket_blocks() {
    for (open, close, tag) in [
        ("(", ")", BracketTag::List),
        ("{", "}", BracketTag::Code),
        ("[", "]", BracketTag::Map),
    ] {
        let src = format!("{open}a{close}");
        let (result, _source) = tokenize_str(&src);
        assert!(
            result.errors.is_empty(),
            "unexpected errors for {src:?}: {:?}",
            result.errors
        );
        assert_eq!(result.result.len(), 1);

        let SyntaxNode::Block(block) = &result.result[0] else {
            panic!("expected a block for {src:?}, got {:?}", result.result[0]);
        };
        assert_eq!(block.start, open);
        assert_eq!(block.end, close);
        assert_eq!(block.tag, tag);
        assert_eq!(block.items.len(), 1);
        assert!(matches!(&block.items[0], SyntaxNode::Identifier(t) if t.str == "a"));
    }
}
