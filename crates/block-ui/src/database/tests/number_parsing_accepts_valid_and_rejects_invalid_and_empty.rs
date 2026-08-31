use super::*;

#[test]
fn number_parsing_accepts_valid_and_rejects_invalid_and_empty() {
    let field = field(DatabaseFieldType::Number);
    assert_eq!(
        parse_cell_value("12.5", &field),
        Some(DatabaseValue::Number(12.5))
    );
    assert_eq!(parse_cell_value("invalid", &field), None);
    assert_eq!(parse_cell_value("", &field), None);
}
