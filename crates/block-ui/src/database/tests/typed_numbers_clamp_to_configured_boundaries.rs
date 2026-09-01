use super::*;

#[test]
fn typed_numbers_clamp_to_configured_boundaries() {
    let mut number = field(DatabaseFieldType::Number);
    number.number_options = DatabaseNumberOptions {
        minimum: Some(10.0),
        maximum: Some(20.0),
        step: Some(2.0),
        scale: DatabaseNumberScale::Linear,
    };
    assert_eq!(
        parse_cell_value("5", &number),
        Some(DatabaseValue::Number(10.0))
    );
    assert_eq!(
        parse_cell_value("15", &number),
        Some(DatabaseValue::Number(15.0))
    );
    assert_eq!(
        parse_cell_value("25", &number),
        Some(DatabaseValue::Number(20.0))
    );
}
