use block::Block;
use block_client::blocks::infinite_canvas::{
    CanvasColor, CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint, CanvasTransform,
    InfiniteCanvas, InfiniteCanvasOperation,
};
use uuid::Uuid;

fn entity(id: Uuid, x: f32) -> CanvasEntity {
    CanvasEntity {
        id,
        transform: CanvasTransform::new(
            CanvasPoint::new(x, 0.0),
            CanvasPoint::new(10.0, 10.0),
            0.0,
        ),
        kind: CanvasEntityKind::Rectangle,
        style: CanvasEntityStyle::default(),
    }
}

#[test]
fn infinite_canvas_applies_entity_changes() {
    let id = Uuid::new_v4();
    let missing = Uuid::new_v4();
    let mut canvas = InfiniteCanvas::new();
    let initial = entity(id, 1.0);

    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::Add {
            entity: initial.clone(),
        },
    );
    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::Add {
            entity: initial.clone(),
        },
    );
    assert_eq!(canvas.entities(), &[initial]);

    let mut updated = entity(id, 9.0);
    updated.style.foreground = CanvasColor::Rgba {
        red: 12,
        green: 34,
        blue: 56,
        alpha: 78,
    };
    updated.style.fill = Some(CanvasColor::Rgba {
        red: 78,
        green: 90,
        blue: 123,
        alpha: 145,
    });
    updated.style.line_width = 7.0;
    updated.style.corner_radius = 11.0;
    updated.style.opacity = 0.4;
    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::Update {
            entities: vec![entity(missing, 3.0), updated.clone()],
        },
    );
    assert_eq!(canvas.entities(), &[updated]);

    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::Remove {
            ids: vec![missing, id],
        },
    );
    assert!(canvas.entities().is_empty());
}
