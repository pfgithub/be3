use super::*;

#[test]
fn screen_world_round_trip_and_cursor_zoom_are_stable() {
    let rect = egui::Rect::from_min_size(egui::pos2(20.0, 40.0), egui::vec2(800.0, 600.0));
    let mut camera = Camera {
        center: [12.0, -8.0],
        zoom: 20.0,
    };
    let cursor = egui::pos2(187.0, 249.0);
    let before = camera.screen_to_world(cursor, rect);
    camera.zoom_around(cursor, rect, 2.0);
    let after = camera.screen_to_world(cursor, rect);
    assert!((before[0] - after[0]).abs() < 0.0001);
    assert!((before[1] - after[1]).abs() < 0.0001);
}
