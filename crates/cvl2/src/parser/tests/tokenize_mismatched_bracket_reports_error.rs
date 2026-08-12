use super::*;

#[test]
fn tokenize_mismatched_bracket_reports_error() {
    let (result, _source) = tokenize_str("(a]");

    assert_eq!(result.errors.len(), 2);
    assert_eq!(
        result.errors[0].entries[0].message,
        "open bracket missing close bracket"
    );
    assert_eq!(result.errors[1].entries[0].message, "extra close bracket");
}
