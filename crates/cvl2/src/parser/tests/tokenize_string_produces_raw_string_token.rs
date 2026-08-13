use super::*;

#[test]
fn tokenize_string_produces_raw_string_token() {
    let (result, _source) = tokenize_str("\"hello\"");
    assert!(result.errors.is_empty());
    assert_eq!(result.result.len(), 1);

    let SyntaxNode::Block(block) = &result.result[0] else {
        panic!("expected a block, got {:?}", result.result[0]);
    };
    assert_eq!(block.start, "\"");
    assert_eq!(block.tag, BracketTag::String);
    assert_eq!(block.items.len(), 1);
    match &block.items[0] {
        SyntaxNode::Raw(r) => {
            assert_eq!(r.raw, "hello");
            assert_eq!(r.tag, RawTag::String);
        }
        other => panic!("expected a raw string token, got {other:?}"),
    }
}
