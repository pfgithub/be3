use super::*;

#[test]
fn invalid_typed_values_do_not_produce_replacements() {
    let color_field = field(DatabaseFieldType::Color);
    assert_eq!(parse_cell_value("#102030", &color_field), None);
    assert_eq!(parse_cell_value("#GG203040", &color_field), None);

    let datetime_field = field(DatabaseFieldType::Datetime);
    assert_eq!(parse_cell_value("2023-02-29 23:45", &datetime_field), None);
    assert_eq!(parse_cell_value("2024-02-29 24:00", &datetime_field), None);

    let mut logarithmic = field(DatabaseFieldType::Number);
    logarithmic.number_options.scale = DatabaseNumberScale::Logarithmic;
    assert_eq!(parse_cell_value("0", &logarithmic), None);
    assert_eq!(parse_cell_value("-1", &logarithmic), None);

    let block = field(DatabaseFieldType::Block);
    assert_eq!(parse_cell_value(&Uuid::new_v4().to_string(), &block), None);
}
