use super::super::*;

#[test]
fn ignores_second_touch_while_first_is_active() {
    let mut ui = TabletUi::new();
    ui.set_page(Page::Notes);
    let size = Vector::new(900.0, 520.0);

    assert!(ui.touch_input(size, 1, TouchPhase::Started, Vector::new(120.0, 160.0),));
    assert!(!ui.touch_input(size, 2, TouchPhase::Started, Vector::new(300.0, 260.0),));
    assert!(!ui.touch_input(size, 2, TouchPhase::Moved, Vector::new(320.0, 280.0),));
    assert!(ui.touch_input(size, 1, TouchPhase::Moved, Vector::new(140.0, 180.0),));
    assert!(!ui.touch_input(size, 2, TouchPhase::Ended, Vector::new(320.0, 280.0),));
    assert!(ui.touch_input(size, 1, TouchPhase::Ended, Vector::new(140.0, 180.0),));

    assert_eq!(ui.active_touch_id, None);
    let points = ui.notes.stroke_points(0).expect("stroke should exist");
    assert_eq!(ui.notes.stroke_count(), 1);
    assert_eq!(points.len(), 2);
    assert_eq!(points[1], Vector::new(140.0, 180.0));
}
