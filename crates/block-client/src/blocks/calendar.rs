use std::time::{Duration, Instant};

use block::{Block, BlockHistory, HistoryDirection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const EDIT_BURST_DELAY: Duration = Duration::from_millis(750);

/// A scheduled event with a start and end time, in whole seconds since the
/// Unix epoch (UTC).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CalendarEvent {
    pub id: Uuid,
    pub title: String,
    pub start: i64,
    pub end: i64,
}

impl CalendarEvent {
    pub fn new(title: String, start: i64, end: i64) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            start,
            end,
        }
    }
}

/// A calendar of events, viewable by day, week, or month.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Calendar {
    events: Vec<CalendarEvent>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum CalendarOperation {
    AddEvent { event: CalendarEvent },
    UpdateEvent { event: CalendarEvent },
    RemoveEvent { id: Uuid },
}

pub struct CalendarHistory;

pub struct CalendarHistoryAction {
    kind: CalendarHistoryActionKind,
    recorded_at: Instant,
}

enum CalendarHistoryActionKind {
    Add(CalendarEvent),
    Remove(CalendarEvent),
    Update {
        before: CalendarEvent,
        after: CalendarEvent,
    },
}

impl Calendar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> &[CalendarEvent] {
        &self.events
    }

    pub fn event(&self, id: Uuid) -> Option<&CalendarEvent> {
        self.events.iter().find(|event| event.id == id)
    }
}

impl Block for Calendar {
    type Operation = CalendarOperation;
    type History = CalendarHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6361_6c65_6e64_6172_2d62_6c6f_636b_0001);
    const CRDT: bool = true;

    fn apply_operation(calendar: &mut Self, operation: &Self::Operation) {
        match operation {
            CalendarOperation::AddEvent { event } => {
                if calendar
                    .events
                    .iter()
                    .any(|existing| existing.id == event.id)
                {
                    return;
                }
                calendar.events.push(normalized_event(event.clone()));
            }
            CalendarOperation::UpdateEvent { event } => {
                if let Some(existing) = calendar
                    .events
                    .iter_mut()
                    .find(|existing| existing.id == event.id)
                {
                    *existing = normalized_event(event.clone());
                }
            }
            CalendarOperation::RemoveEvent { id } => {
                calendar.events.retain(|event| event.id != *id);
            }
        }
    }
}

/// Ensures an event never ends before it starts.
fn normalized_event(mut event: CalendarEvent) -> CalendarEvent {
    if event.end < event.start {
        event.end = event.start;
    }
    event
}

impl BlockHistory<Calendar> for CalendarHistory {
    type Action = CalendarHistoryAction;
    type Snapshot = Calendar;

    fn snapshot(block: &Calendar) -> Self::Snapshot {
        block.clone()
    }

    fn action(
        before: Calendar,
        after: &Calendar,
        operations: &[CalendarOperation],
    ) -> Option<Self::Action> {
        let operation = operations.last()?;
        let kind = match operation {
            CalendarOperation::AddEvent { event } => {
                if before.event(event.id).is_some() {
                    return None;
                }
                CalendarHistoryActionKind::Add(after.event(event.id)?.clone())
            }
            CalendarOperation::RemoveEvent { id } => {
                CalendarHistoryActionKind::Remove(before.event(*id)?.clone())
            }
            CalendarOperation::UpdateEvent { event } => {
                let previous = before.event(event.id)?.clone();
                let current = after.event(event.id)?.clone();
                if previous == current {
                    return None;
                }
                CalendarHistoryActionKind::Update {
                    before: previous,
                    after: current,
                }
            }
        };
        Some(CalendarHistoryAction {
            kind,
            recorded_at: Instant::now(),
        })
    }

    fn action_bytes(action: &Self::Action) -> usize {
        match &action.kind {
            CalendarHistoryActionKind::Add(event) | CalendarHistoryActionKind::Remove(event) => {
                size_of::<CalendarEvent>() + event.title.len()
            }
            CalendarHistoryActionKind::Update { before, after } => {
                size_of::<CalendarEvent>() * 2 + before.title.len() + after.title.len()
            }
        }
    }

    fn merge(previous: &mut Self::Action, next: Self::Action) -> Result<(), Self::Action> {
        if next.recorded_at.duration_since(previous.recorded_at) > EDIT_BURST_DELAY {
            return Err(next);
        }
        match (&mut previous.kind, &next.kind) {
            (
                CalendarHistoryActionKind::Update {
                    after: previous_after,
                    ..
                },
                CalendarHistoryActionKind::Update {
                    after: next_after, ..
                },
            ) if previous_after.id == next_after.id => {
                previous_after.clone_from(next_after);
            }
            _ => return Err(next),
        }
        previous.recorded_at = next.recorded_at;
        Ok(())
    }

    fn operations(
        current: &Calendar,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) -> Vec<CalendarOperation> {
        let to_after = direction == HistoryDirection::Redo;
        match &action.kind {
            CalendarHistoryActionKind::Add(event) => {
                if to_after {
                    vec![CalendarOperation::AddEvent {
                        event: event.clone(),
                    }]
                } else {
                    vec![CalendarOperation::RemoveEvent { id: event.id }]
                }
            }
            CalendarHistoryActionKind::Remove(event) => {
                if to_after {
                    vec![CalendarOperation::RemoveEvent { id: event.id }]
                } else {
                    vec![CalendarOperation::AddEvent {
                        event: event.clone(),
                    }]
                }
            }
            CalendarHistoryActionKind::Update { before, after } => {
                let (expected, desired) = if to_after {
                    (before, after)
                } else {
                    (after, before)
                };
                let Some(current) = current.event(expected.id) else {
                    return Vec::new();
                };
                vec![CalendarOperation::UpdateEvent {
                    event: rebase_event(current.clone(), expected.clone(), desired.clone()),
                }]
            }
        }
    }
}

/// Keeps fields a concurrent editor changed while restoring the fields this
/// history action owns.
fn rebase_event(
    current: CalendarEvent,
    expected: CalendarEvent,
    desired: CalendarEvent,
) -> CalendarEvent {
    let mut result = current;
    if result.title == expected.title {
        result.title = desired.title;
    }
    if result.start == expected.start {
        result.start = desired.start;
    }
    if result.end == expected.end {
        result.end = desired.end;
    }
    result
}

#[cfg(test)]
mod tests;
