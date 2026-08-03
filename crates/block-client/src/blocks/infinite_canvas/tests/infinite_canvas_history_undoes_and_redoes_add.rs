use uuid::Uuid;

use super::{
    CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint, CanvasTransform,
    InfiniteCanvas, InfiniteCanvasOperation,
};
use crate::BlockClient;

#[test]
fn infinite_canvas_history_undoes_and_redoes_add() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(InfiniteCanvas::new());
    let entity = CanvasEntity {
        id: Uuid::new_v4(),
        transform: CanvasTransform::new(
            CanvasPoint::new(1.0, 2.0),
            CanvasPoint::new(10.0, 20.0),
            0.0,
        ),
        kind: CanvasEntityKind::Rectangle,
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
    };
    block.operate(InfiniteCanvasOperation::Add {
        entity: entity.clone(),
    });
    assert_eq!(block.read().unwrap().entities(), &[entity.clone()]);
    block.undo();
    assert!(block.read().unwrap().entities().is_empty());
    block.redo();
    assert_eq!(block.read().unwrap().entities(), &[entity]);
}
