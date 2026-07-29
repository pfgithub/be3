use block_client::blocks::infinite_canvas::{
    CanvasEntity, CanvasEntityKind, CanvasPoint, CanvasTransform, InfiniteCanvas,
    InfiniteCanvasOperation,
};
use uuid::Uuid;

#[test]
fn infinite_canvas_serialization_round_trips() {
    let entity = CanvasEntity {
        id: Uuid::new_v4(),
        transform: CanvasTransform::new(
            CanvasPoint::new(10.0, -4.0),
            CanvasPoint::new(120.0, 80.0),
            0.25,
        ),
        kind: CanvasEntityKind::Pen {
            points: vec![CanvasPoint::new(-0.5, 0.0), CanvasPoint::new(0.5, 1.0)],
        },
    };
    let canvas = InfiniteCanvas::new();
    let operation = InfiniteCanvasOperation::Add { entity };

    let canvas_json = serde_json::to_vec(&canvas).unwrap();
    let operation_json = serde_json::to_vec(&operation).unwrap();

    assert_eq!(
        serde_json::from_slice::<InfiniteCanvas>(&canvas_json).unwrap(),
        canvas
    );
    assert_eq!(
        serde_json::from_slice::<InfiniteCanvasOperation>(&operation_json).unwrap(),
        operation
    );
}
