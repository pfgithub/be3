use super::*;

#[test]
fn child_view_changes_round_trip() {
    for change in [
        ViewChange::Pan { x: 3.0, y: -4.0 },
        ViewChange::Zoom {
            factor: 1.5,
            anchor: Some((10.0, 20.0)),
        },
        ViewChange::Fit,
    ] {
        let message = Message::Editor(EditorMessage::ChildView {
            instance: EditorInstanceId(2),
            region: EditorRegion::Frame,
            child: ChildId(7),
            change,
        });
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
