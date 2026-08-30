use super::*;

#[test]
fn adding_a_field_appends_a_string_field() {
    let (mut editor, block) = editor();

    editor.find("database-schema.add-field").click();
    editor.run();

    let schema = block.read().unwrap();
    let fields = schema.fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "Field");
    drop(schema);
    editor.snapshot("adding_a_field_appends_a_string_field");
}
