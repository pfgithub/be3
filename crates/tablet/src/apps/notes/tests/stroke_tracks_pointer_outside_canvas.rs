use super::super::*;

#[test]
fn stroke_tracks_pointer_outside_canvas() {
    let size = Vector::new(900.0, 520.0);
    let mut app = NotesApp::new();

    assert!(app.pointer_pressed(size, Vector::new(200.0, 200.0)));
    assert!(app.pointer_moved(size, Vector::new(-20.0, 220.0)));
    assert!(app.pointer_moved(size, Vector::new(200.0, 300.0)));
    assert!(app.pointer_released(size, Vector::new(200.0, 300.0)));

    assert_eq!(app.coverage_at(size, Vector::new(200.0, 250.0)), 0);
    assert_eq!(
        app.coverage_at(size, Vector::new(200.0, 300.0)),
        PEN_COVERAGE
    );
}
