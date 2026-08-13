use super::*;

#[test]
fn get_comptime_reports_kind_mismatch() {
    let mut env = new_env();
    let v = RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid));

    let err = get_comptime(&mut env, Some(ComptimeValueKind::Ast), v, pos_at(0)).unwrap_err();

    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };
    assert_eq!(e.entries[0].message, "Expected value of type Ast, got Void");
}
