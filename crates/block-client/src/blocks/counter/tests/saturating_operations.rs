use super::*;

#[test]
fn saturating_operations() {
    let mut maximum: Counter = serde_json::from_str(r#"{"count":9223372036854775807}"#).unwrap();
    Counter::apply_operation(&mut maximum, &CounterOperation::Increment);
    assert_eq!(maximum.count(), i64::MAX);

    let mut minimum: Counter = serde_json::from_str(r#"{"count":-9223372036854775808}"#).unwrap();
    Counter::apply_operation(&mut minimum, &CounterOperation::Decrement);
    assert_eq!(minimum.count(), i64::MIN);
}
