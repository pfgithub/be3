use super::*;

#[test]
fn switching_to_kanban_stores_the_kind() {
    let (mut editor, block, _, _) = editor();

    editor.find("database-view.kind.Kanban").click();
    editor.run();

    assert_eq!(block.read().unwrap().kind(), DatabaseViewKind::Kanban);
    editor.snapshot("switching_to_kanban_stores_the_kind");
}
