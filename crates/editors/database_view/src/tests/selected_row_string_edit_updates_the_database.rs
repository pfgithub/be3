use super::*;

#[test]
fn selected_row_string_edit_updates_the_database() {
    let (mut editor, _, database, field_id) = editor();

    editor
        .find(&format!("database-view.cell.0.{field_id}"))
        .click();
    editor.run();
    editor
        .find(&format!("database-view.selected-item.field.{field_id}"))
        .click();
    editor
        .find(&format!("database-view.selected-item.field.{field_id}"))
        .type_text("alpha");
    editor.run();

    assert_eq!(
        database.read().unwrap().rows()[0].value(field_id),
        Some(&DatabaseValue::String("alpha".to_owned()))
    );
    editor.snapshot("selected_row_string_edit_updates_the_database");
}
