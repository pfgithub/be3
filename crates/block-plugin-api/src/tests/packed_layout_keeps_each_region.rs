use super::*;

#[test]
fn packed_layout_keeps_each_region() {
    let screens = [
        region_screen(EditorRegion::Preview, 1, 7, 400, 30, 1.0),
        region_screen(EditorRegion::ArtifactSettings, 2, 7, 200, 300, 1.0),
        region_screen(EditorRegion::Frame, 3, 7, 400, 300, 1.0),
    ];
    let layout = ScreenLayout::packed(&screens);
    assert_eq!(layout.width, 400);
    assert_eq!(layout.height, 630);
    for placement in &layout.screens {
        assert_eq!(placement.instance, EditorInstanceId(7));
    }
    for (screen, region) in [
        (1, EditorRegion::Preview),
        (2, EditorRegion::ArtifactSettings),
        (3, EditorRegion::Frame),
    ] {
        assert_eq!(
            layout
                .placement(ScreenId(screen))
                .map(|placement| placement.region),
            Some(region)
        );
    }
    for (index, placement) in layout.screens.iter().enumerate() {
        for other in &layout.screens[index + 1..] {
            let overlaps = placement.x < other.x + other.width
                && other.x < placement.x + placement.width
                && placement.y < other.y + other.height
                && other.y < placement.y + placement.height;
            assert!(!overlaps, "{placement:?} overlaps {other:?}");
        }
    }
}
