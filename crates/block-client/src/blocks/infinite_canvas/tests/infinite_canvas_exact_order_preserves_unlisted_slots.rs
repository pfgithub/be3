use block::Block;
use uuid::Uuid;

use super::{
    CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint, CanvasTransform,
    InfiniteCanvas, InfiniteCanvasOperation,
};

fn entity(id: Uuid) -> CanvasEntity {
    CanvasEntity {
        id,
        transform: CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(1.0, 1.0), 0.0),
        kind: CanvasEntityKind::Rectangle,
        style: CanvasEntityStyle::default(),
    }
}

#[test]
fn infinite_canvas_exact_order_preserves_unlisted_slots() {
    let ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let remote = Uuid::new_v4();
    let mut canvas = InfiniteCanvas::new();
    for id in [ids[0], remote, ids[1], ids[2]] {
        InfiniteCanvas::apply_operation(
            &mut canvas,
            &InfiniteCanvasOperation::Add { entity: entity(id) },
        );
    }
    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::ExactOrder {
            ids: vec![ids[2], ids[0], ids[1]],
        },
    );
    let actual = canvas
        .entities()
        .iter()
        .map(|entity| entity.id)
        .collect::<Vec<_>>();
    assert_eq!(actual, [ids[2], remote, ids[0], ids[1]]);
}
