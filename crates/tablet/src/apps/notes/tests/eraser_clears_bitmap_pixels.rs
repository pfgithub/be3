use super::super::*;

#[test]
fn eraser_clears_bitmap_pixels() {
    let size = Vector::new(900.0, 520.0);
    let position = Vector::new(180.0, 160.0);
    let mut app = NotesApp::new();

    assert!(app.pointer_pressed(size, position));
    assert!(app.pointer_released(size, position));
    assert_eq!(app.coverage_at(size, position), PEN_COVERAGE);

    app.selected_tool = Tool::Eraser;
    assert!(app.pointer_pressed(size, position));
    assert!(app.pointer_released(size, position));

    assert_eq!(app.coverage_at(size, position), 0);
}
