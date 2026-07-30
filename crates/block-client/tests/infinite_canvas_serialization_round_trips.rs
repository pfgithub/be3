use block_client::blocks::infinite_canvas::{
    CanvasColor, CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint, CanvasTransform,
    InfiniteCanvas, InfiniteCanvasOperation,
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
        style: CanvasEntityStyle {
            foreground: CanvasColor::Rgb {
                red: 10,
                green: 20,
                blue: 30,
            },
            line_width: 6.5,
            dashed: true,
            fill: Some(CanvasColor::Rgb {
                red: 40,
                green: 50,
                blue: 60,
            }),
            arrow_start: true,
            arrow_end: true,
            corner_radius: 14.0,
            opacity: 0.35,
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
