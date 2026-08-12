use super::*;

#[test]
fn tokenize_access_and_builtin_identifiers() {
    let (result, _source) = tokenize_str(".foo #bar");
    assert!(result.errors.is_empty());
    assert_eq!(result.result.len(), 3);

    match &result.result[0] {
        SyntaxNode::Identifier(t) => {
            assert_eq!(t.str, "foo");
            assert_eq!(t.ident_tag, IdentifierTag::Access);
            assert_eq!(t.ident_tag_raw, ".");
        }
        other => panic!("expected identifier, got {other:?}"),
    }

    match &result.result[2] {
        SyntaxNode::Identifier(t) => {
            assert_eq!(t.str, "bar");
            assert_eq!(t.ident_tag, IdentifierTag::Builtin);
            assert_eq!(t.ident_tag_raw, "#");
        }
        other => panic!("expected identifier, got {other:?}"),
    }
}
