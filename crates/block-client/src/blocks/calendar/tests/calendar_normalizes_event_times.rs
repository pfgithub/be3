use block::Block;

use super::{Calendar, CalendarEvent, CalendarOperation};

#[test]
fn calendar_normalizes_event_times() {
    let mut calendar = Calendar::new();
    let event = CalendarEvent::new("Backwards".into(), 2_000, 1_000);
    Calendar::apply_operation(
        &mut calendar,
        &CalendarOperation::AddEvent {
            event: event.clone(),
        },
    );
    let stored = calendar.event(event.id).expect("event was added");
    assert_eq!(stored.start, 2_000);
    assert_eq!(stored.end, 2_000);

    let mut updated = stored.clone();
    updated.end = 500;
    Calendar::apply_operation(
        &mut calendar,
        &CalendarOperation::UpdateEvent { event: updated },
    );
    let stored = calendar.event(event.id).expect("event still exists");
    assert_eq!(stored.start, 2_000);
    assert_eq!(stored.end, 2_000);
}
