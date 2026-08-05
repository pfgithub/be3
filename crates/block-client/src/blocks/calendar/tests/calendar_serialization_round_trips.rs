use block::Block;

use super::{Calendar, CalendarEvent, CalendarOperation};

#[test]
fn calendar_serialization_round_trips() {
    let mut calendar = Calendar::new();
    let event = CalendarEvent::new("Standup".into(), 1_000, 2_000);
    Calendar::apply_operation(
        &mut calendar,
        &CalendarOperation::AddEvent {
            event: event.clone(),
        },
    );

    let json = serde_json::to_string(&calendar).expect("calendar serializes");
    let restored: Calendar = serde_json::from_str(&json).expect("calendar deserializes");
    assert_eq!(restored, calendar);

    let operation = CalendarOperation::UpdateEvent { event };
    let json = serde_json::to_string(&operation).expect("operation serializes");
    let restored: CalendarOperation = serde_json::from_str(&json).expect("operation deserializes");
    assert_eq!(restored, operation);
}
