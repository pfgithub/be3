use super::*;

#[test]
fn validate_c_name_accepts_valid_identifiers() {
    assert!(validate_c_name("_foo"));
    assert!(validate_c_name("foo_bar123"));
    assert!(validate_c_name("A"));
    assert!(CValidatedIdentifierName::new("valid_name".to_string()).is_some());
}
