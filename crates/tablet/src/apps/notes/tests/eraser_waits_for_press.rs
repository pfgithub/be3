use super::super::*;
use super::{atlas, test_atlas};

#[test]
fn eraser_waits_for_press() {
    let size = Vector::new(900.0, 520.0);
    let position = Vector::new(180.0, 160.0);
    let mut app = NotesApp::new();
    let mut pixels = test_atlas();
    app.pointer_pressed(size, position, &mut atlas(&mut pixels));
    app.pointer_released(size, position, &mut atlas(&mut pixels));

    app.selected_tool = Tool::Eraser;
    let changed = app.pointer_moved(size, position, &mut atlas(&mut pixels));

    assert!(!changed);
    assert_eq!(
        app.coverage_at(size, position, &atlas(&mut pixels)),
        PEN_COVERAGE
    );
}
