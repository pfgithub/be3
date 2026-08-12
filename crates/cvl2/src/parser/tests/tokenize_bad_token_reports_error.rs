use super::*;

#[test]
fn tokenize_bad_token_reports_error() {
    let (result, _source) = tokenize_str("#");

    assert!(result.result.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].entries[0].message, "bad token \"#\"");
    assert_eq!(result.errors[0].entries[0].style, ErrorStyle::Error);
}
