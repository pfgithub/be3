use super::*;

#[test]
fn comptime_eval_ns_list_append_duplicate_reports_error() {
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
            AnalysisLine::ComptimeAst {
                pos: pos_at(4),
                narrow: comptime_ast(4),
            },
            AnalysisLine::ComptimeNsListAppend {
                pos: pos_at(5),
                list: BlockIdx(0),
                key: BlockIdx(1),
                value: BlockIdx(4),
            },
        ],
    };

    let Err(err) = comptime_eval(&mut env, &block) else {
        panic!("expected a duplicate-declaration error");
    };

    assert_eq!(err.e.entries[0].message, "already declared");
    assert_eq!(err.e.entries[1].message, "previous definition here");
    assert_eq!(err.e.entries[1].pos, Some(pos_at(2)));
}
