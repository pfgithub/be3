use super::*;

#[test]
fn escape_only_leaves_a_frame_that_has_a_trail() {
    let size = egui::vec2(1200.0, 800.0);
    let escape = vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }];
    assert!(!show(size, frame, escape.clone()).exit);
    let nested = || frame().trail(vec!["Canvas".to_owned(), "Spreadsheet".to_owned()]);
    assert!(show(size, nested, escape).exit);
    assert!(!show(size, nested, Vec::new()).exit);
}
