use super::super::*;

#[test]
fn eraser_waits_for_press() {
    let size = Vector::new(900.0, 520.0);
    let position = Vector::new(180.0, 160.0);
    let mut app = NotesApp::new();
    app.pointer_pressed(size, position);
    app.pointer_released(size, position);

    app.selected_tool = Tool::Eraser;
    let changed = app.pointer_moved(size, position);

    assert!(!changed);
    assert_eq!(app.coverage_at(size, position), PEN_COVERAGE);
}
