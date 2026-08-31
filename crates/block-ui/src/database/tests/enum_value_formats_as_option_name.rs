use super::*;

#[test]
fn enum_value_formats_as_option_name() {
    let option = DatabaseEnumOption {
        id: Uuid::new_v4(),
        name: "Alpha".to_owned(),
    };
    let mut field = field(DatabaseFieldType::Enum);
    field.options.push(option.clone());
    assert_eq!(
        database_value_text(&DatabaseValue::Enum(option.id), &field),
        option.name
    );
}
