use super::super::*;

#[test]
fn pen_rasterizes_line_into_bitmap() {
    let size = Vector::new(900.0, 520.0);
    let mut app = NotesApp::new();

    assert!(app.pointer_pressed(size, Vector::new(120.0, 160.0)));
    assert!(app.pointer_moved(size, Vector::new(180.0, 190.0)));
    assert!(app.pointer_released(size, Vector::new(180.0, 190.0)));

    assert_eq!(
        app.coverage_at(size, Vector::new(120.0, 160.0)),
        PEN_COVERAGE
    );
    assert_eq!(
        app.coverage_at(size, Vector::new(150.0, 175.0)),
        PEN_COVERAGE
    );
    assert_eq!(
        app.coverage_at(size, Vector::new(180.0, 190.0)),
        PEN_COVERAGE
    );
}
