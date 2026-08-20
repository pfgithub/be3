use super::*;

#[test]
fn stacked_layout_stacks_screens() {
    let screens = [
        screen(1, 1, 300, 200, 2.0),
        screen(2, 2, 0, 0, 1.0),
        screen(3, 3, 500, 100, 1.0),
    ];
    let layout = ScreenLayout::stacked(&screens);
    assert_eq!(layout.width, 500);
    assert_eq!(layout.height, 300);
    assert_eq!(layout.screens.len(), 2);
    assert_eq!(
        layout.placement(ScreenId(1)),
        Some(&ScreenPlacement {
            screen: ScreenId(1),
            instance: EditorInstanceId(1),
            x: 0,
            y: 0,
            width: 300,
            height: 200,
            scale_factor_millis: 2000,
        })
    );
    assert_eq!(
        layout.placement(ScreenId(3)),
        Some(&ScreenPlacement {
            screen: ScreenId(3),
            instance: EditorInstanceId(3),
            x: 0,
            y: 200,
            width: 500,
            height: 100,
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
