use super::*;

#[test]
fn tokenize_raw_tokens() {
    let (result, _source) = tokenize_str("->");
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    assert_eq!(result.result.len(), 1);
    match &result.result[0] {
        SyntaxNode::Raw(t) => {
            assert_eq!(t.raw, "->");
            assert_eq!(t.tag, RawTag::Return);
        }
        other => panic!("expected raw token, got {other:?}"),
    }
}
