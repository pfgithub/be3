use super::*;

#[test]
#[should_panic(expected = "todo: comptime eval expr: call")]
fn comptime_eval_unhandled_line_panics() {
    let mut env = new_env();
    let block = AnalysisBlock {
        lines: vec![AnalysisLine::Call {
            pos: pos_at(0),
            method: BlockIdx(0),
            arg: BlockIdx(0),
        }],
    };

    let _ = comptime_eval(&mut env, &block);
}
