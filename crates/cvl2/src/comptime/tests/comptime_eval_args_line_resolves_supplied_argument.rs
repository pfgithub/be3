use super::*;

#[test]
fn comptime_eval_args_line_resolves_supplied_argument() {
    let validate = Symbol::new();
    let block = AnalysisBlock {
        offset: 0,
        validate,
        lines: vec![AnalysisLine::Args { pos: pos_at(0) }],
    };
    let mut env = new_env();
    let supplied = RuntimeValue::Comptime(ComptimeValue::Key(ComptimeValueKey::String {
        key: "arg".to_string(),
    }));

    let result = comptime_eval_with_args(
        &mut env,
        &block,
        RuntimeValue::Runtime(BlockIdx(0, validate)),
        pos_at(1),
        Some(supplied),
    )
    .unwrap();

    let ComptimeValue::Key(ComptimeValueKey::String { key }) = result else {
        panic!("expected a string key");
    };
    assert_eq!(key, "arg");
}
