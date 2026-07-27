use super::super::*;

#[test]
fn touch_input_draws_notes_stroke() {
    let mut ui = TabletUi::new();
    ui.set_page(Page::Notes);
    let size = Vector::new(900.0, 520.0);

    assert!(ui.touch_input(size, 7, TouchPhase::Started, Vector::new(120.0, 160.0),));
    assert!(ui.touch_input(size, 7, TouchPhase::Moved, Vector::new(180.0, 190.0),));
    assert!(ui.touch_input(size, 7, TouchPhase::Ended, Vector::new(180.0, 190.0),));

    assert_eq!(ui.active_touch_id, None);
    assert_eq!(ui.notes.coverage_at(size, Vector::new(120.0, 160.0)), 255);
    assert_eq!(ui.notes.coverage_at(size, Vector::new(150.0, 175.0)), 255);
    assert_eq!(ui.notes.coverage_at(size, Vector::new(180.0, 190.0)), 255);
}
