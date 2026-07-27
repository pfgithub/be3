use super::super::*;
use super::{atlas, test_atlas};

#[test]
fn eraser_clears_bitmap_pixels() {
    let size = Vector::new(900.0, 520.0);
    let position = Vector::new(180.0, 160.0);
    let mut app = NotesApp::new();
    let mut pixels = test_atlas();

    assert!(app.pointer_pressed(size, position, &mut atlas(&mut pixels)));
    assert!(app.pointer_released(size, position, &mut atlas(&mut pixels)));
    assert_eq!(
        app.coverage_at(size, position, &atlas(&mut pixels)),
        PEN_COVERAGE
    );

    app.selected_tool = Tool::Eraser;
    assert!(app.pointer_pressed(size, position, &mut atlas(&mut pixels)));
    assert!(app.pointer_released(size, position, &mut atlas(&mut pixels)));

    assert_eq!(app.coverage_at(size, position, &atlas(&mut pixels)), 0);
}
