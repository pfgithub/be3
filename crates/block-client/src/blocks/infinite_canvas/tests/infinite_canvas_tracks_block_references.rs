use super::*;

#[test]
fn infinite_canvas_tracks_block_references() {
    let [first, second] = std::array::from_fn(|_| Uuid::new_v4());
    let [a, b, direct] = std::array::from_fn(|_| Uuid::new_v4());
    let mut canvas = InfiniteCanvas::new();

    let mut first_entity = block_entity(a, BlockRef::Direct(first));
    first_entity.components.push(CanvasComponent {
        schema_id: BlockRef::Direct(first),
        values: BTreeMap::new(),
    });
    for entity in [first_entity, block_entity(b, BlockRef::Direct(first))] {
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
            entities: vec![block_entity(a, BlockRef::Direct(second))],
        },
    );
    assert_eq!(canvas.references(), vec![second, first]);

    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::Remove { ids: vec![b] },
    );
    assert_eq!(canvas.references(), vec![second]);
}
