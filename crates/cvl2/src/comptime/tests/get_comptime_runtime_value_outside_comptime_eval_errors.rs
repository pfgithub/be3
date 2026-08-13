use super::*;

#[test]
fn get_comptime_runtime_value_outside_comptime_eval_errors() {
    let mut env = new_env();
    let v = RuntimeValue::Runtime(BlockIdx(0, Symbol::new()));

    let err = get_comptime(&mut env, None, v, pos_at(0)).unwrap_err();

    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };
    assert_eq!(e.entries[0].message, "Value must be known at comptime.");
}
