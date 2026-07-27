use super::super::*;
use super::{atlas, test_atlas};

#[test]
fn ignores_second_touch_while_first_is_active() {
    let mut ui = TabletUi::new();
    ui.set_page(Page::Notes);
    let size = Vector::new(900.0, 520.0);
    let mut pixels = test_atlas();

    assert!(ui.touch_input(
        size,
        1,
        TouchPhase::Started,
        Vector::new(120.0, 160.0),
        &mut atlas(&mut pixels),
    ));
    assert!(!ui.touch_input(
        size,
        2,
        TouchPhase::Started,
        Vector::new(300.0, 260.0),
        &mut atlas(&mut pixels),
    ));
    assert!(!ui.touch_input(
        size,
        2,
        TouchPhase::Moved,
        Vector::new(320.0, 280.0),
        &mut atlas(&mut pixels),
    ));
    assert!(ui.touch_input(
        size,
        1,
        TouchPhase::Moved,
        Vector::new(140.0, 180.0),
        &mut atlas(&mut pixels),
    ));
    assert!(!ui.touch_input(
        size,
        2,
        TouchPhase::Ended,
        Vector::new(320.0, 280.0),
        &mut atlas(&mut pixels),
    ));
    assert!(ui.touch_input(
        size,
        1,
        TouchPhase::Ended,
        Vector::new(140.0, 180.0),
        &mut atlas(&mut pixels),
    ));

    assert_eq!(ui.active_touch_id, None);
    assert_eq!(
        ui.notes
            .coverage_at(size, Vector::new(140.0, 180.0), &atlas(&mut pixels)),
        255
    );
    assert_eq!(
        ui.notes
            .coverage_at(size, Vector::new(300.0, 260.0), &atlas(&mut pixels)),
        0
    );
}
