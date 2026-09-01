use super::*;

#[test]
fn frame_screens_and_reports_round_trip() {
    let mut request = region_screen(EditorRegion::Frame, 1, 2, 640, 480, 1.0);
    request.frame = Some(FrameSpec {
        chrome: FrameChrome::Drawn,
        content: Some(ChildRect {
            x: 10.0,
            y: 20.0,
            width: 300.0,
            height: 200.0,
        }),
        trail: vec!["Canvas".to_owned(), "Spreadsheet".to_owned()],
    });
    let screens = Message::Screens(ScreenSet {
        request_id: 3,
        screens: vec![request],
    });
    assert_eq!(
        decode_frame(&encode_frame(&screens).unwrap()).unwrap(),
        screens
    );

    let frames = Message::Frames(vec![FrameReport {
        screen: ScreenId(1),
        content: ChildRect {
            x: 0.0,
            y: 40.0,
            width: 640.0,
            height: 440.0,
        },
        painted: vec![ChildRect {
            x: 0.0,
            y: 0.0,
            width: 640.0,
            height: 40.0,
        }],
        floating: vec![ChildRect {
            x: 8.0,
            y: 44.0,
            width: 120.0,
            height: 90.0,
        }],
    }]);
    assert_eq!(
        decode_frame(&encode_frame(&frames).unwrap()).unwrap(),
        frames
    );
}
