use super::History;

#[test]
fn history_evicts_oldest_entries() {
    let mut history = History::default();
    for value in 0..101 {
        history.push(value, 1);
    }
    let mut undone = Vec::new();
    while history.can_undo() {
        history.undo(|value| undone.push(*value));
    }
    assert_eq!(undone.len(), 100);
    assert_eq!(undone.last(), Some(&1));
}
