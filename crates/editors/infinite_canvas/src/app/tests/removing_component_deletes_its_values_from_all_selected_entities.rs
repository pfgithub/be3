use super::*;

#[test]
fn removing_component_deletes_its_values_from_all_selected_entities() {
    let schema_id = BlockRef::Direct(Uuid::new_v4());
    let field_id = Uuid::new_v4();
    let [first_id, second_id] = std::array::from_fn(|_| Uuid::new_v4());
    let mut entities = [entity(first_id), entity(second_id)];
    let selected = HashSet::from([first_id, second_id]);
    attach_component(&mut entities, &selected, schema_id);
    set_component_value(
        &mut entities,
        &selected,
        schema_id,
        field_id,
        Some(DatabaseValue::Number(2.0)),
    );

    remove_component(&mut entities, &selected, schema_id);

    assert!(entities.iter().all(|entity| entity.components.is_empty()));
}
