use super::*;

#[test]
fn tokenize_string_interpolation_opens_list_block() {
    let (result, _source) = tokenize_str(r#""a\(b)c""#);
    assert!(result.errors.is_empty());
    assert_eq!(result.result.len(), 1);

    let SyntaxNode::Block(string_block) = &result.result[0] else {
        panic!("expected a block, got {:?}", result.result[0]);
    };
    assert_eq!(string_block.tag, BracketTag::String);
    assert_eq!(string_block.items.len(), 3);

    match &string_block.items[0] {
        SyntaxNode::Raw(r) => {
            assert_eq!(r.raw, "a");
            assert_eq!(r.tag, RawTag::String);
        }
        other => panic!("expected a raw string token, got {other:?}"),
    }

    match &string_block.items[1] {
        SyntaxNode::Block(interp) => {
            assert_eq!(interp.start, "\\(");
            assert_eq!(interp.end, ")");
            assert_eq!(interp.tag, BracketTag::List);
            assert!(
                matches!(&interp.items[..], [SyntaxNode::Identifier(t)] if t.str == "b"),
                "expected a single identifier `b`, got {:?}",
                interp.items
            );
        }
        other => panic!("expected an interpolation block, got {other:?}"),
    }

    match &string_block.items[2] {
        SyntaxNode::Raw(r) => {
            assert_eq!(r.raw, "c");
            assert_eq!(r.tag, RawTag::String);
        }
        other => panic!("expected a raw string token, got {other:?}"),
    }
}
