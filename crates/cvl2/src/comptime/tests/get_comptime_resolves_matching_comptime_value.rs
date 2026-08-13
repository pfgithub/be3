use super::*;

#[test]
fn get_comptime_resolves_matching_comptime_value() {
    let mut env = new_env();
    let v = RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid));

    let result = get_comptime(&mut env, Some(ComptimeValueKind::Void), v, pos_at(0)).unwrap();

    assert!(matches!(result, ComptimeValue::Void(_)));
}
