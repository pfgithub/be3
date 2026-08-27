use super::*;

#[test]
fn show_region_messages_round_trip() {
    for region in EditorRegion::ALL {
        for shown in [true, false] {
            let message = Message::Editor(EditorMessage::ShowRegion {
                instance: EditorInstanceId(11),
                region,
                shown,
            });
            assert_eq!(
                decode_frame(&encode_frame(&message).unwrap()).unwrap(),
                message
            );
        }
    }
}
