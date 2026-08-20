use super::*;

#[test]
fn stacked_layout_keeps_each_region() {
    let screens = [
        region_screen(EditorRegion::Toolbar, 1, 7, 400, 30, 1.0),
        region_screen(EditorRegion::LeftSidebar, 2, 7, 200, 300, 1.0),
        region_screen(EditorRegion::Main, 3, 7, 400, 300, 1.0),
    ];
    let layout = ScreenLayout::stacked(&screens);
    assert_eq!(layout.width, 400);
    assert_eq!(layout.height, 630);
    let regions: Vec<_> = layout
        .screens
        .iter()
        .map(|placement| (placement.region, placement.y, placement.height))
        .collect();
    assert_eq!(
        regions,
        vec![
            (EditorRegion::Toolbar, 0, 30),
            (EditorRegion::LeftSidebar, 30, 300),
            (EditorRegion::Main, 330, 300),
        ]
    );
    for placement in &layout.screens {
        assert_eq!(placement.instance, EditorInstanceId(7));
    }
    assert_eq!(
        layout
            .placement(ScreenId(2))
            .map(|placement| placement.region),
        Some(EditorRegion::LeftSidebar)
    );
}
