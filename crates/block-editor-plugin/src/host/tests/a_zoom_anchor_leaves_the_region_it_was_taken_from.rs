use super::*;

#[test]
fn a_zoom_anchor_leaves_the_region_it_was_taken_from() {
    let host = EditorHost::default();
    host.begin_region(EditorRegion::Frame, egui::vec2(1000.0, 500.0));
    host.zoom_view(2.0, Some(egui::pos2(1030.0, 520.0)));
    host.zoom_view(0.5, None);
    assert_eq!(
        host.take_view_changes(),
        vec![
            ViewChange::Zoom {
                factor: 2.0,
                anchor: Some((30.0, 20.0)),
            },
            ViewChange::Zoom {
                factor: 0.5,
                anchor: None,
            },
        ]
    );
}
