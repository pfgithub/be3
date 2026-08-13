use super::*;

#[test]
fn comptime_eval_kv_list_append_rejects_locked_fields() {
    let validate = Symbol::new();
    let block = AnalysisBlock {
        offset: 0,
        validate,
        lines: vec![
            AnalysisLine::Args { pos: pos_at(0) },
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
    let locked_fields = RuntimeValue::Comptime(ComptimeValue::KvFields(NsFields {
        locked: true,
        entries: Vec::new(),
    }));

    let err = comptime_eval_with_args(
        &mut env,
        &block,
        RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
        pos_at(2),
        Some(locked_fields),
    )
    .unwrap_err();

    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };
    assert_eq!(e.entries[0].message, "assertion failed");
    assert_eq!(e.entries[0].pos, Some(pos_at(1)));
}
