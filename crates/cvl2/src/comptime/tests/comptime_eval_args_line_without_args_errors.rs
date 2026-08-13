use super::*;

#[test]
fn comptime_eval_args_line_without_args_errors() {
    let validate = Symbol::new();
    let block = AnalysisBlock {
        offset: 0,
        validate,
        lines: vec![AnalysisLine::Args { pos: pos_at(0) }],
    };
    let mut env = new_env();

    let err = comptime_eval(
        &mut env,
        &block,
        RuntimeValue::Runtime(BlockIdx(0, validate)),
        pos_at(1),
    )
    .unwrap_err();

    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };
    assert_eq!(
        e.entries[0].message,
        "cannot get args when executing without args"
    );
    assert_eq!(e.entries[1].message, "called here");
    assert_eq!(e.entries[1].pos, Some(pos_at(1)));
}
