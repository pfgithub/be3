use super::*;

#[test]
fn packed_layout_packs_screens_within_a_row() {
    let screens = [
        screen(1, 1, 100, 200, 2.0),
        screen(2, 2, 0, 0, 1.0),
        screen(3, 3, 100, 200, 1.0),
    ];
    let layout = ScreenLayout::packed(&screens);
    assert_eq!(layout.width, 200);
    assert_eq!(layout.height, 200);
    assert_eq!(layout.screens.len(), 2);
    assert_eq!(
        layout.placement(ScreenId(1)),
        Some(&ScreenPlacement {
            screen: ScreenId(1),
            instance: EditorInstanceId(1),
            region: EditorRegion::Frame,
            x: 0,
            y: 0,
            width: 100,
            height: 200,
            scale_factor_millis: 2000,
        })
    );
    assert_eq!(
        layout.placement(ScreenId(3)),
        Some(&ScreenPlacement {
            screen: ScreenId(3),
            instance: EditorInstanceId(3),
            region: EditorRegion::Frame,
            x: 100,
            y: 0,
            width: 100,
            height: 200,
            scale_factor_millis: 1000,
        })
    );
    assert_eq!(layout.placement(ScreenId(2)), None);
    assert_eq!(layout.placement(ScreenId(1)).unwrap().scale_factor(), 2.0);
    assert!(layout.same_placements(&ScreenLayout {
        generation: 7,
        ..layout.clone()
    }));
}
