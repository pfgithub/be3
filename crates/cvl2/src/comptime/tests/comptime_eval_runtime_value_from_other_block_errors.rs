use super::*;

#[test]
fn comptime_eval_runtime_value_from_other_block_errors() {
    let block = AnalysisBlock {
        offset: 0,
        validate: Symbol::new(),
        lines: Vec::new(),
    };
    let mut env = new_env();
    let foreign_idx = BlockIdx(0, Symbol::new());

    let err = comptime_eval(
        &mut env,
        &block,
        RuntimeValue::Runtime(foreign_idx),
        pos_at(0),
    )
    .unwrap_err();

    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };
    assert_eq!(e.entries[0].message, "Assertion failure: Ex");
}
