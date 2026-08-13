use super::*;

#[test]
fn comptime_eval_kv_list_init_and_append_builds_fields() {
    let validate = Symbol::new();
    let block = AnalysisBlock {
        offset: 0,
        validate,
        lines: vec![
            AnalysisLine::ComptimeKvListInit { pos: pos_at(0) },
            AnalysisLine::ComptimeKvListAppend {
                pos: pos_at(1),
                list: RuntimeValue::Runtime(BlockIdx(0, validate)),
                key: RuntimeValue::Comptime(ComptimeValue::Key(ComptimeValueKey::String {
                    key: "field".to_string(),
                })),
                value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
            },
        ],
    };
    let mut env = new_env();

    let result = comptime_eval(
        &mut env,
        &block,
        RuntimeValue::Runtime(BlockIdx(0, validate)),
        pos_at(2),
    )
    .unwrap();

    let ComptimeValue::KvFields(fields) = result else {
        panic!("expected comptime:kv_fields result");
    };
    assert!(!fields.locked);
    assert_eq!(fields.entries.len(), 1);
    assert_eq!(fields.entries[0].pos, pos_at(1));
    let ComptimeValue::Key(ComptimeValueKey::String { key }) = &fields.entries[0].key else {
        panic!("expected a string key");
    };
    assert_eq!(key, "field");
}
