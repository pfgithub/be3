use block::Block;

use super::{Calendar, CalendarEvent, CalendarOperation};

#[test]
fn calendar_adds_updates_and_removes_events() {
    let mut calendar = Calendar::new();
    let event = CalendarEvent::new("Standup".into(), 1_000, 2_000);
    Calendar::apply_operation(
        &mut calendar,
        &CalendarOperation::AddEvent {
            event: event.clone(),
        },
    );
                                                      
    Calendar::apply_operation(
        &mut calendar,
        &CalendarOperation::AddEvent {
            event: event.clone(),
        },
    );
    assert_eq!(calendar.events(), std::slice::from_ref(&event));

    let mut renamed = event.clone();
    renamed.title = "Daily standup".into();
    renamed.end = 2_500;
    Calendar::apply_operation(
        &mut calendar,
        &CalendarOperation::UpdateEvent {
            event: renamed.clone(),
        },
    );
    assert_eq!(calendar.event(event.id), Some(&renamed));

                                                                      
    Calendar::apply_operation(
        &mut calendar,
        &CalendarOperation::UpdateEvent {
            event: CalendarEvent::new("Ghost".into(), 0, 0),
        },
    );
    assert_eq!(calendar.events(), &[renamed]);

    Calendar::apply_operation(
        &mut calendar,
        &CalendarOperation::RemoveEvent { id: event.id },
    );
    assert!(calendar.events().is_empty());
}
