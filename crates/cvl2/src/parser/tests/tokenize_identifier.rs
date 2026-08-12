use super::*;

#[test]
fn tokenize_identifier() {
    let (result, _source) = tokenize_str("abc123");
    assert!(result.errors.is_empty());
    assert_eq!(result.result.len(), 1);
    match &result.result[0] {
        SyntaxNode::Identifier(t) => {
            assert_eq!(t.str, "abc123");
            assert_eq!(t.ident_tag, IdentifierTag::Normal);
            assert_eq!(t.ident_tag_raw, "");
        }
        other => panic!("expected identifier, got {other:?}"),
    }
}
