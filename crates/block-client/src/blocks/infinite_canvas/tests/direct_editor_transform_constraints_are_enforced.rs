use super::{
    CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint, CanvasTransform,
    InfiniteCanvas, InfiniteCanvasOperation,
};
use block::Block;
use uuid::Uuid;

#[test]
fn direct_editor_transform_constraints_are_enforced() {
    let id = Uuid::new_v4();
    let mut canvas = InfiniteCanvas::new();
    let entity = CanvasEntity {
        id,
        transform: CanvasTransform::new(
            CanvasPoint::default(),
            CanvasPoint::new(200.0, 100.0),
            1.5,
        ),
        kind: CanvasEntityKind::DirectEditor {
            block_id: Uuid::new_v4(),
            scale: -2.0,
        },
        style: CanvasEntityStyle::default(),
        group_id: None,
    };

    InfiniteCanvas::apply_operation(&mut canvas, &InfiniteCanvasOperation::Add { entity });

    assert_eq!(canvas.entities()[0].transform.rotation, 0.0);
    assert!(matches!(
        canvas.entities()[0].kind,
        CanvasEntityKind::DirectEditor { scale: 1.0, .. }
    ));
}
