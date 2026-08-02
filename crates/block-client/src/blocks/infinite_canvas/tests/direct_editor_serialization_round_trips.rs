use super::{
    CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint, CanvasTransform,
    InfiniteCanvasOperation,
};
use uuid::Uuid;

#[test]
fn direct_editor_serialization_round_trips() {
    let operation = InfiniteCanvasOperation::Add {
        entity: CanvasEntity {
            id: Uuid::new_v4(),
            transform: CanvasTransform::new(
                CanvasPoint::new(10.0, 20.0),
                CanvasPoint::new(448.0, 112.0),
                0.0,
            ),
            kind: CanvasEntityKind::DirectEditor {
                block_id: Uuid::new_v4(),
                scale: 2.0,
            },
            style: CanvasEntityStyle::default(),
            group_id: None,
        },
    };

    let json = serde_json::to_vec(&operation).unwrap();

    assert_eq!(
        serde_json::from_slice::<InfiniteCanvasOperation>(&json).unwrap(),
        operation
    );
}
