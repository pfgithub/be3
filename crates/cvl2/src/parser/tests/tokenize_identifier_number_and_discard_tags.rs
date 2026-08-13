use super::*;

#[test]
fn tokenize_identifier_number_and_discard_tags() {
    let (result, _source) = tokenize_str("123abc _ _foo");
    let idents: Vec<&IdentifierToken> = result
        .result
        .iter()
        .filter_map(|n| match n {
            SyntaxNode::Identifier(t) => Some(t),
            _ => None,
        })
        .collect();
    assert_eq!(idents.len(), 3);

    assert_eq!(idents[0].str, "123abc");
    assert_eq!(idents[0].ident_tag, IdentifierTag::Number);

    assert_eq!(idents[1].str, "_");
    assert_eq!(idents[1].ident_tag, IdentifierTag::Discard);

    assert_eq!(idents[2].str, "_foo");
    assert_eq!(idents[2].ident_tag, IdentifierTag::Normal);
}
