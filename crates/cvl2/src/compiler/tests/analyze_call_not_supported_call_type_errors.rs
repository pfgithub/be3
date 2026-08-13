use super::*;

#[test]
fn analyze_call_not_supported_call_type_errors() {
    let mut env = new_env();
    let mut block = empty_block();
    let method = AnalysisResult {
        ty: Type::Void(TypeVoid),
        value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
    };

    let result = analyze_call(
        &mut env,
        Type::Void(TypeVoid),
        pos_at(0),
        method,
        CallArg {
            pos: pos_at(0),
            ast: &[],
        },
        &mut block,
    );
    let Err(err) = result else {
        panic!("expected an error");
    };
    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };

    assert_eq!(e.entries[0].message, "not supported call type: TypeVoid");
}
