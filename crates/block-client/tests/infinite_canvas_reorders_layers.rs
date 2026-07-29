use block::Block;
use block_client::blocks::infinite_canvas::{
    CanvasEntity, CanvasEntityKind, CanvasLayerMove, CanvasPoint, CanvasTransform, InfiniteCanvas,
    InfiniteCanvasOperation,
};
use uuid::Uuid;

fn entity(id: Uuid) -> CanvasEntity {
    CanvasEntity {
        id,
        transform: CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(1.0, 1.0), 0.0),
        kind: CanvasEntityKind::Rectangle,
    }
}

fn canvas(ids: &[Uuid]) -> InfiniteCanvas {
    let mut canvas = InfiniteCanvas::new();
    for id in ids {
        InfiniteCanvas::apply_operation(
            &mut canvas,
            &InfiniteCanvasOperation::Add {
                entity: entity(*id),
            },
        );
    }
    canvas
}

fn ids(canvas: &InfiniteCanvas) -> Vec<Uuid> {
    canvas.entities().iter().map(|entity| entity.id).collect()
}

#[test]
fn infinite_canvas_reorders_layers() {
    let [a, b, c, d] = std::array::from_fn(|_| Uuid::new_v4());
    let selected = vec![a, c];

    let mut forward = canvas(&[a, b, c, d]);
    InfiniteCanvas::apply_operation(
        &mut forward,
        &InfiniteCanvasOperation::Reorder {
            ids: selected.clone(),
            movement: CanvasLayerMove::ForwardOne,
        },
    );
    assert_eq!(ids(&forward), vec![b, a, d, c]);

    let mut back = canvas(&[a, b, c, d]);
    InfiniteCanvas::apply_operation(
        &mut back,
        &InfiniteCanvasOperation::Reorder {
            ids: selected.clone(),
            movement: CanvasLayerMove::BackOne,
        },
    );
    assert_eq!(ids(&back), vec![a, c, b, d]);

    let mut front = canvas(&[a, b, c, d]);
    InfiniteCanvas::apply_operation(
        &mut front,
        &InfiniteCanvasOperation::Reorder {
            ids: selected.clone(),
            movement: CanvasLayerMove::BringToFront,
        },
    );
    assert_eq!(ids(&front), vec![b, d, a, c]);

    let mut sent_back = canvas(&[a, b, c, d]);
    InfiniteCanvas::apply_operation(
        &mut sent_back,
        &InfiniteCanvasOperation::Reorder {
            ids: selected,
            movement: CanvasLayerMove::SendToBack,
        },
    );
    assert_eq!(ids(&sent_back), vec![a, c, b, d]);
}
