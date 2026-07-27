use super::super::*;
use super::{atlas, test_atlas};

#[test]
fn stroke_tracks_pointer_outside_canvas() {
    let size = Vector::new(900.0, 520.0);
    let mut app = NotesApp::new();
    let mut pixels = test_atlas();

    assert!(app.pointer_pressed(size, Vector::new(200.0, 200.0), &mut atlas(&mut pixels)));
    assert!(app.pointer_moved(size, Vector::new(-20.0, 220.0), &mut atlas(&mut pixels)));
    assert!(app.pointer_moved(size, Vector::new(200.0, 300.0), &mut atlas(&mut pixels)));
    assert!(app.pointer_released(size, Vector::new(200.0, 300.0), &mut atlas(&mut pixels)));

    assert_eq!(
        app.coverage_at(size, Vector::new(200.0, 250.0), &atlas(&mut pixels)),
        0
    );
    assert_eq!(
        app.coverage_at(size, Vector::new(200.0, 300.0), &atlas(&mut pixels)),
        PEN_COVERAGE
    );
}
