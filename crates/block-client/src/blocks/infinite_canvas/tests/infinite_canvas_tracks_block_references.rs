use super::{
    CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint, CanvasTransform,
    InfiniteCanvas, InfiniteCanvasOperation,
};
use block::Block;
use uuid::Uuid;

fn block_entity(id: Uuid, block_id: Uuid) -> CanvasEntity {
    CanvasEntity {
        id,
        transform: CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(1.0, 1.0), 0.0),
        kind: CanvasEntityKind::Block { block_id },
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
    }
}

fn direct_editor_entity(id: Uuid, block_id: Uuid) -> CanvasEntity {
    CanvasEntity {
        id,
        transform: CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(1.0, 1.0), 0.0),
        kind: CanvasEntityKind::DirectEditor {
            block_id,
            scale: 1.0,
        },
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
    }
}

#[test]
fn infinite_canvas_tracks_block_references() {
    let [first, second] = std::array::from_fn(|_| Uuid::new_v4());
    let [a, b, direct] = std::array::from_fn(|_| Uuid::new_v4());
    let mut canvas = InfiniteCanvas::new();

    for entity in [block_entity(a, first), block_entity(b, first)] {
        InfiniteCanvas::apply_operation(&mut canvas, &InfiniteCanvasOperation::Add { entity });
    }
    assert_eq!(canvas.references(), vec![first]);

    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::Add {
            entity: direct_editor_entity(direct, second),
        },
    );
    assert_eq!(canvas.references(), vec![first, second]);

    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::Update {
            entities: vec![block_entity(a, second)],
        },
    );
    assert_eq!(canvas.references(), vec![second, first]);

    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::Remove { ids: vec![b] },
    );
    assert_eq!(canvas.references(), vec![second]);
}
