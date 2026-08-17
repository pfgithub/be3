use super::*;

#[test]
fn superseded_events_are_coalesced() {
    let mut session = running_session();
    session
        .enqueue(input(InputEvent::PointerMoved { x: 1.0, y: 2.0 }))
        .unwrap();
    session
        .enqueue(input(InputEvent::PointerMoved { x: 3.0, y: 4.0 }))
        .unwrap();
    session
        .enqueue(input(InputEvent::PointerButton {
            button: PointerButton::Primary,
            pressed: true,
            x: 3.0,
            y: 4.0,
        }))
        .unwrap();
    session
        .enqueue(input(InputEvent::Wheel {
            x: 1.0,
            y: 2.0,
            unit: WheelUnit::Pixels,
        }))
        .unwrap();
    session
        .enqueue(input(InputEvent::Wheel {
            x: 3.0,
            y: 4.0,
            unit: WheelUnit::Pixels,
        }))
        .unwrap();
    session
        .enqueue(input(InputEvent::Modifiers(Modifiers::default())))
        .unwrap();
    session
        .enqueue(input(InputEvent::Modifiers(Modifiers::default())))
        .unwrap();
    assert_eq!(session.queued_message_count(), 4);
    assert_eq!(
        session.next_outbound(),
        Some(input(InputEvent::PointerMoved { x: 3.0, y: 4.0 }))
    );
    assert_eq!(
        session.next_outbound(),
        Some(input(InputEvent::PointerButton {
            button: PointerButton::Primary,
            pressed: true,
            x: 3.0,
            y: 4.0,
        }))
    );
    assert_eq!(
        session.next_outbound(),
        Some(input(InputEvent::Wheel {
            x: 4.0,
            y: 6.0,
            unit: WheelUnit::Pixels,
        }))
    );
}
