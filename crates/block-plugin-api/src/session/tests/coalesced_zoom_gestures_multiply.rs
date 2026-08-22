use super::*;

#[test]
fn coalesced_zoom_gestures_multiply() {
    let mut session = running_session();
    session
        .enqueue(input(InputEvent::Zoom { factor: 2.0 }))
        .unwrap();
    session
        .enqueue(input(InputEvent::Zoom { factor: 1.5 }))
        .unwrap();

    assert_eq!(session.queued_message_count(), 1);
    assert_eq!(
        session.next_outbound(),
        Some(input(InputEvent::Zoom { factor: 3.0 }))
    );
}
