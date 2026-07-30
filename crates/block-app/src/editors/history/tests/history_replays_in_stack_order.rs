use super::History;

#[test]
fn history_replays_in_stack_order() {
    let mut history = History::default();
    history.push(1, 1);
    history.push(2, 1);
    let mut values = Vec::new();
    history.undo(|value| values.push(*value));
    history.undo(|value| values.push(*value));
    history.redo(|value| values.push(*value));
    assert_eq!(values, [2, 1, 1]);
}
