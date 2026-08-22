use super::*;

#[test]
fn tile_region_survives_rounding_at_the_padded_edge() {
    let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(612.0, 792.0));
    let visible = Rect::from_min_max(
        Pos2::new(142.647_69, 73.984_66),
        Pos2::new(251.005_43, 252.979_77),
    );

    let region = tile_region(visible, bounds, 4.0);

    assert!(bounds.contains_rect(region), "{region:?}");
    assert!(region.contains(visible.center()), "{region:?}");
}
