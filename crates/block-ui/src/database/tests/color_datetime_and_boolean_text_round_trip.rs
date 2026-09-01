use super::*;

#[test]
fn color_datetime_and_boolean_text_round_trip() {
    let color_field = field(DatabaseFieldType::Color);
    let color = DatabaseValue::Color(DatabaseColor {
        red: 0x10,
        green: 0x20,
        blue: 0x30,
        alpha: 0x40,
    });
    assert_eq!(
        database_value_text(&color, &color_field, &HashMap::new()),
        "#10203040"
    );
    assert_eq!(parse_cell_value("#10203040", &color_field), Some(color));

    let datetime_field = field(DatabaseFieldType::Datetime);
    let datetime = parse_cell_value("2024-02-29 23:45", &datetime_field).unwrap();
    assert_eq!(
        database_value_text(&datetime, &datetime_field, &HashMap::new()),
        "2024-02-29 23:45"
    );

    let boolean_field = field(DatabaseFieldType::Boolean);
    for text in ["true", "false"] {
        let value = parse_cell_value(text, &boolean_field).unwrap();
        assert_eq!(
            database_value_text(&value, &boolean_field, &HashMap::new()),
            text
        );
    }
}
