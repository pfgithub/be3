use super::*;

#[test]
fn attaching_component_fills_only_missing_selected_entities() {
    let schema_id = BlockRef::Direct(Uuid::new_v4());
    let [first_id, second_id] = std::array::from_fn(|_| Uuid::new_v4());
    let mut entities = [entity(first_id), entity(second_id)];
    entities[0].components.push(CanvasComponent {
        schema_id,
        values: BTreeMap::new(),
    });
    let selected = HashSet::from([first_id, second_id]);

    attach_component(&mut entities, &selected, schema_id);

    assert_eq!(entities[0].components.len(), 1);
    assert_eq!(entities[1].components.len(), 1);
    assert_eq!(entities[1].components[0].schema_id, schema_id);
}
