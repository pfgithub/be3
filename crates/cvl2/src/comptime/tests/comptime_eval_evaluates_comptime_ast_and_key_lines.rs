use super::*;

#[test]
fn comptime_eval_evaluates_comptime_ast_and_key_lines() {
    let mut env = new_env();
    let block = AnalysisBlock {
        lines: vec![
            AnalysisLine::ComptimeAst {
                pos: pos_at(0),
                narrow: comptime_ast(0),
            },
            AnalysisLine::ComptimeKey {
                pos: pos_at(1),
                narrow: string_key("field"),
            },
        ],
    };

    let results = comptime_eval(&mut env, &block).unwrap();

    let ast = results[0].downcast_ref::<ComptimeValueAst>().unwrap();
    assert_eq!(ast.pos, pos_at(0));

    let key = results[1].downcast_ref::<ComptimeNarrowKey>().unwrap();
    match key {
        ComptimeNarrowKey::String { key } => assert_eq!(key, "field"),
        ComptimeNarrowKey::Symbol { .. } => panic!("expected a string key"),
    }
}
