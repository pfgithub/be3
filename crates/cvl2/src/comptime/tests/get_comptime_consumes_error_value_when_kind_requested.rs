use super::*;

#[test]
fn get_comptime_consumes_error_value_when_kind_requested() {
    let mut env = new_env();
    let v = RuntimeValue::Comptime(ComptimeValue::Error(ComptimeValueError {
        etok: ConsumedErrorToken,
    }));

    let err = get_comptime(&mut env, Some(ComptimeValueKind::Void), v, pos_at(0)).unwrap_err();

    assert!(matches!(err, PositionedError::Consumed));
}
