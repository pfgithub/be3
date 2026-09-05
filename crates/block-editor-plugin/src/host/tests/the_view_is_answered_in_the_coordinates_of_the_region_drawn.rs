use super::*;

#[test]
fn the_view_is_answered_in_the_coordinates_of_the_region_drawn() {
    let host = EditorHost::default();
    host.set_view(
        egui::Rect::from_min_size(egui::pos2(30.0, 20.0), egui::vec2(100.0, 50.0)),
        2.0,
    );
    assert_eq!(
        host.view(),
        Some(egui::Rect::from_min_size(
            egui::pos2(30.0, 20.0),
            egui::vec2(100.0, 50.0)
        ))
    );

    assert_eq!(host.view_scale(), Some(2.0));

    host.begin_region(EditorRegion::Frame, egui::vec2(1000.0, 500.0));
    assert_eq!(
        host.view(),
        Some(egui::Rect::from_min_size(
            egui::pos2(1030.0, 520.0),
            egui::vec2(100.0, 50.0)
        ))
    );
}
