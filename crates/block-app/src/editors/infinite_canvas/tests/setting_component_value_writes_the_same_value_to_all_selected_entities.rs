use super::*;

#[test]
fn setting_component_value_writes_the_same_value_to_all_selected_entities() {
    let schema_id = BlockRef::Direct(Uuid::new_v4());
    let [first_id, second_id, unselected_id] = std::array::from_fn(|_| Uuid::new_v4());
    let mut entities = [entity(first_id), entity(second_id), entity(unselected_id)];
    let selected = HashSet::from([first_id, second_id]);
    attach_component(&mut entities, &selected, schema_id);
    let values = [
        DatabaseValue::String("alpha".to_owned()),
        DatabaseValue::Boolean(false),
        DatabaseValue::Block(BlockRef::Direct(Uuid::new_v4())),
        DatabaseValue::Datetime(-1),
    ];

    for value in values {
        let field_id = Uuid::new_v4();
        set_component_value(
            &mut entities,
            &selected,
            schema_id,
            field_id,
            Some(value.clone()),
        );
        assert_eq!(
            entities[0].components[0].values.get(&field_id),
            Some(&value)
        );
        assert_eq!(
            entities[1].components[0].values.get(&field_id),
            Some(&value)
        );
        assert!(entities[2].components.is_empty());
        set_component_value(&mut entities, &selected, schema_id, field_id, None);
        assert_eq!(entities[0].components[0].values.get(&field_id), None);
        assert_eq!(entities[1].components[0].values.get(&field_id), None);
    }
}
