use super::*;

// A lone "_" is no longer reachable as a `RawTag::Discard` token: since the
// identifier regex now includes `_`, it is caught by the identifier branch
// first and tagged `IdentifierTag::Discard` instead (see
// tokenize_identifier_number_and_discard_tags).
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
