use super::*;

#[test]
fn validate_c_name_rejects_invalid_identifiers() {
    assert!(!validate_c_name(""));
    assert!(!validate_c_name("1abc"));
    assert!(!validate_c_name("has space"));
    assert!(!validate_c_name("has-dash"));
    assert!(CValidatedIdentifierName::new("1bad".to_string()).is_none());
}
