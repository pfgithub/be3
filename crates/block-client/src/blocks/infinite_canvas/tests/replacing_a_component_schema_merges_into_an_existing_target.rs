use super::*;

#[test]
fn replacing_a_component_schema_merges_into_an_existing_target() {
    let [old, new, collision_field, old_only_field] =
        std::array::from_fn(|_| Uuid::new_v4());
    let mut entity = block_entity(Uuid::new_v4(), BlockRef::Direct(old));
    entity.components = vec![
        CanvasComponent {
            schema_id: BlockRef::Direct(old),
            values: BTreeMap::from([
                (
                    collision_field,
                    DatabaseValue::String("old collision".to_owned()),
                ),
                (
                    old_only_field,
                    DatabaseValue::String("old only".to_owned()),
                ),
            ]),
        },
        CanvasComponent {
            schema_id: BlockRef::Direct(new),
            values: BTreeMap::from([(
                collision_field,
                DatabaseValue::String("new collision".to_owned()),
            )]),
        },
    ];
    let mut canvas = InfiniteCanvas::new();
    InfiniteCanvas::apply_operation(&mut canvas, &InfiniteCanvasOperation::Add { entity });

    for operation in canvas.replace_child(old, new).unwrap() {
        InfiniteCanvas::apply_operation(&mut canvas, &operation);
    }

    assert_eq!(canvas.references(), vec![new]);
    let entity = &canvas.entities()[0];
    assert!(matches!(
        entity.kind,
        CanvasEntityKind::Block {
            block_id: BlockRef::Direct(id)
        } if id == new
    ));
    assert_eq!(entity.components.len(), 1);
    assert_eq!(
        entity.components[0].values.get(&collision_field),
        Some(&DatabaseValue::String("new collision".to_owned()))
    );
    assert_eq!(
        entity.components[0].values.get(&old_only_field),
        Some(&DatabaseValue::String("old only".to_owned()))
    );
}
