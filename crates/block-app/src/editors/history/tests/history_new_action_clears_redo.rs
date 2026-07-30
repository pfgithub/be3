use super::History;

#[test]
fn history_new_action_clears_redo() {
    let mut history = History::default();
    history.push(1, 1);
    history.undo(|_| {});
    assert!(history.can_redo());
    history.push(2, 1);
    assert!(!history.can_redo());
}
