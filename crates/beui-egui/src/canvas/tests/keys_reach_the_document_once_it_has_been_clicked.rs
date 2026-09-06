use super::*;

#[test]
fn keys_reach_the_document_once_it_has_been_clicked() {
    let mut harness = harness();

    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(clicks(&harness), 0);

    let center = button_center(&harness);
    click(&mut harness, center);
    harness.key_press(egui::Key::Tab);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();

    assert_eq!(clicks(&harness), 2);
}
