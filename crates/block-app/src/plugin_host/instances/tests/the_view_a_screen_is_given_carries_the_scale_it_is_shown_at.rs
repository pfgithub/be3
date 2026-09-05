use super::*;

#[test]
fn the_view_a_screen_is_given_carries_the_scale_it_is_shown_at() {
    let (mut instances, _context, _id) = placed();
    instances.next_screens(PASS);
    instances.set_view(
        INSTANCE,
        EditorView {
            rect: egui::Rect::from_min_size(egui::pos2(4.0, 6.0), egui::vec2(200.0, 100.0)),
            scale: 0.5,
        },
    );

    let opened = instances.next_screens(PASS).opened;

    assert!(opened.iter().any(|message| matches!(
        message,
        Message::Editor(EditorMessage::ViewChanged {
            instance,
            x,
            y,
            width,
            height,
            scale,
        }) if *instance == INSTANCE
            && *x == 4.0
            && *y == 6.0
            && *width == 200.0
            && *height == 100.0
            && *scale == 0.5
    )));
}
