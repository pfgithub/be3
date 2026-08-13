use super::*;

#[test]
fn comptime_eval_ns_list_init_and_append_registers_field() {
    let mut env = new_env();
    let block = AnalysisBlock {
        lines: vec![
            AnalysisLine::ComptimeNsListInit { pos: pos_at(0) },
            AnalysisLine::ComptimeKey {
                pos: pos_at(1),
                narrow: string_key("field"),
            },
            AnalysisLine::ComptimeAst {
                pos: pos_at(2),
                narrow: comptime_ast(2),
            },
            AnalysisLine::ComptimeNsListAppend {
                pos: pos_at(3),
                list: BlockIdx(0),
                key: BlockIdx(1),
                value: BlockIdx(2),
            },
        ],
    };

    let results = comptime_eval(&mut env, &block).unwrap();

    let fields = results[0].downcast_ref::<NsFields>().unwrap();
    assert!(!fields.locked);
    let entry = fields
        .registered
        .get(&NsKey::Str("field".to_string()))
        .unwrap();
    assert_eq!(entry.ast.pos, pos_at(2));
}
