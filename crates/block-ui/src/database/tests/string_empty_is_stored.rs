use super::*;

#[test]
fn string_empty_is_stored() {
    let field = field(DatabaseFieldType::String);
    assert_eq!(
        parse_cell_value("", &field),
        Some(DatabaseValue::String(String::new()))
    );
}
