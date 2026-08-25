use super::*;

#[test]
fn outbound_transport_does_not_saturate_the_session_queue() {
    let mut session = running_session();
    let mut writer = Writer::default();
    let count = MAX_QUEUED_MESSAGES + 1;

    for index in 0..count {
        let message = Message::Input(InputBatch {
            screen: ScreenId(1),
            events: vec![InputEvent::Text(index.to_string())],
        });
        send_outbound(&mut writer, &mut session, message, 2).unwrap();
    }

    assert_eq!(writer.messages, count);
    assert_eq!(session.queued_message_count(), 0);
}
