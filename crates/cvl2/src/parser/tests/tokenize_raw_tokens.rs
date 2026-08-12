use super::*;

#[test]
fn tokenize_raw_tokens() {
    for (src, tag) in [("->", RawTag::Return), ("_", RawTag::Discard)] {
        let (result, _source) = tokenize_str(src);
        assert!(
            result.errors.is_empty(),
            "unexpected errors for {src:?}: {:?}",
            result.errors
        );
        assert_eq!(result.result.len(), 1);
        match &result.result[0] {
            SyntaxNode::Raw(t) => {
                assert_eq!(t.raw, src);
                assert_eq!(t.tag, tag);
            }
            other => panic!("expected raw token for {src:?}, got {other:?}"),
        }
    }
}
