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
    let points = ui.notes.stroke_points(0).expect("stroke should exist");
    assert_eq!(ui.notes.stroke_count(), 1);
    assert_eq!(points.len(), 2);
    assert_eq!(points[0], Vector::new(120.0, 160.0));
    assert_eq!(points[1], Vector::new(180.0, 190.0));
}
