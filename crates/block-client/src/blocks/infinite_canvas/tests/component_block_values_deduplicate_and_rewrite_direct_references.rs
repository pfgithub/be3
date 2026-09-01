use super::*;

#[test]
fn component_block_values_deduplicate_and_rewrite_direct_references() {
    let entity_block = Uuid::new_v4();
    let schema = Uuid::new_v4();
    let old = Uuid::new_v4();
    let new = Uuid::new_v4();
    let field = Uuid::new_v4();
    let repo_field = Uuid::new_v4();
    let repo_relative = BlockRef::RepoRelative {
        repo: Uuid::new_v4(),
        eternal_id: Uuid::new_v4(),
    };
    let mut entity = block_entity(Uuid::new_v4(), BlockRef::Direct(entity_block));
    entity.components.push(CanvasComponent {
        schema_id: BlockRef::Direct(schema),
        values: BTreeMap::from([
            (field, DatabaseValue::Block(BlockRef::Direct(old))),
            (repo_field, DatabaseValue::Block(repo_relative)),
        ]),
    });
    let mut canvas = InfiniteCanvas::new();
    InfiniteCanvas::apply_operation(&mut canvas, &InfiniteCanvasOperation::Add { entity });
    assert_eq!(canvas.references(), vec![entity_block, schema, old]);

    for operation in canvas.replace_child(old, new).unwrap() {
        InfiniteCanvas::apply_operation(&mut canvas, &operation);
    }
    assert_eq!(canvas.references(), vec![entity_block, schema, new]);
    assert_eq!(
        canvas.entities()[0].components[0].values.get(&field),
        Some(&DatabaseValue::Block(BlockRef::Direct(new)))
    );
    assert_eq!(
        canvas.entities()[0].components[0].values.get(&repo_field),
        Some(&DatabaseValue::Block(repo_relative))
    );

    for operation in canvas.delete_child(new).unwrap() {
        InfiniteCanvas::apply_operation(&mut canvas, &operation);
    }
    assert_eq!(canvas.references(), vec![entity_block, schema]);
    assert_eq!(canvas.entities()[0].components[0].values.get(&field), None);
}
