use super::*;

#[test]
fn gui_builder_history_undoes_and_redoes_changes() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(GuiBuilder::new());
    let group = container();
    let text = label("Nested");
    let (group_id, text_id) = (group.id, text.id);

    block.operate(GuiBuilderOperation::Insert {
        location: GuiLocation::new(None, 0),
        widget: group,
    });
    block.operate(GuiBuilderOperation::Insert {
        location: GuiLocation::new(Some(group_id), 0),
        widget: text,
    });
    block.operate(GuiBuilderOperation::Move {
        id: text_id,
        location: GuiLocation::new(None, 0),
    });
    block.operate(GuiBuilderOperation::Remove { id: group_id });
    assert_eq!(
        block.read().unwrap().location(text_id),
        Some(GuiLocation::new(None, 0))
    );
    assert!(block.read().unwrap().widget(group_id).is_none());

    block.undo();
    assert_eq!(
        block.read().unwrap().location(group_id),
        Some(GuiLocation::new(None, 1))
    );
    block.undo();
    assert_eq!(
        block.read().unwrap().location(text_id),
        Some(GuiLocation::new(Some(group_id), 0))
    );

    block.redo();
    block.redo();
    assert!(block.read().unwrap().widget(group_id).is_none());
    assert_eq!(
        block.read().unwrap().location(text_id),
        Some(GuiLocation::new(None, 0))
    );
}
