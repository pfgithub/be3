use uuid::Uuid;

use super::{Calendar, CalendarEvent, CalendarOperation};
use crate::BlockClient;

#[test]
fn calendar_history_undoes_and_redoes_events() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(Calendar::new());
    let event = CalendarEvent::new("Standup".into(), 1_000, 2_000);
    block.operate(CalendarOperation::AddEvent {
        event: event.clone(),
    });

    let mut renamed = event.clone();
    renamed.title = "Daily standup".into();
    block.operate(CalendarOperation::UpdateEvent {
        event: renamed.clone(),
    });
    block.operate(CalendarOperation::RemoveEvent { id: event.id });
    assert!(block.read().unwrap().events().is_empty());

    block.undo();
    assert_eq!(block.read().unwrap().event(event.id), Some(&renamed));
    block.undo();
    assert_eq!(block.read().unwrap().event(event.id), Some(&event));
    block.undo();
    assert!(block.read().unwrap().events().is_empty());

    block.redo();
    assert_eq!(block.read().unwrap().event(event.id), Some(&event));
    block.redo();
    assert_eq!(block.read().unwrap().event(event.id), Some(&renamed));
    block.redo();
    assert!(block.read().unwrap().events().is_empty());
}
