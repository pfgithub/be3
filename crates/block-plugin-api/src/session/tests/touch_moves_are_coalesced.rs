use super::*;

#[test]
fn touch_moves_are_coalesced() {
    let mut session = running_session();
    session
        .enqueue(input(InputEvent::Touch {
            device: 2,
            finger: 3,
            phase: crate::TouchPhase::Move,
            x: 10.0,
            y: 20.0,
            force: Some(0.4),
        }))
        .unwrap();
    session
        .enqueue(input(InputEvent::Touch {
            device: 2,
            finger: 3,
            phase: crate::TouchPhase::Move,
            x: 30.0,
            y: 40.0,
            force: Some(0.8),
        }))
        .unwrap();

    assert_eq!(session.queued_message_count(), 1);
    assert_eq!(
        session.next_outbound(),
        Some(input(InputEvent::Touch {
            device: 2,
            finger: 3,
            phase: crate::TouchPhase::Move,
            x: 30.0,
            y: 40.0,
            force: Some(0.8),
        }))
    );
}
