use super::*;

#[test]
fn value_state_distinguishes_uniform_absent_and_mixed_values() {
    let field_id = Uuid::new_v4();
    let empty = BTreeMap::new();
    let mut alpha = BTreeMap::new();
    alpha.insert(field_id, DatabaseValue::String("alpha".to_owned()));
    let mut alpha_too = BTreeMap::new();
    alpha_too.insert(field_id, DatabaseValue::String("alpha".to_owned()));

    assert_eq!(
        value_state(&[&empty, &empty], field_id),
        ValueState::Uniform(None)
    );
    assert_eq!(
        value_state(&[&alpha, &alpha_too], field_id),
        ValueState::Uniform(alpha.get(&field_id))
    );
    assert_eq!(value_state(&[&empty, &alpha], field_id), ValueState::Mixed);
}
