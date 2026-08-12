use super::*;

#[test]
fn tokenize_whitespace_and_newline() {
    let (result, _source) = tokenize_str("a  b");
    assert!(result.errors.is_empty());
    assert_eq!(result.result.len(), 3);
    match &result.result[1] {
        SyntaxNode::Whitespace(w) => assert!(!w.nl),
        other => panic!("expected whitespace, got {other:?}"),
    }
}
