use super::*;

#[test]
fn tile_region_matches_visible_bounds() {
    let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(612.0, 792.0));
    let visible = Rect::from_min_max(
        Pos2::new(142.647_69, 73.984_66),
        Pos2::new(251.005_43, 252.979_77),
    );

    let region = tile_region(visible, bounds);

    assert_eq!(region, visible);
}
