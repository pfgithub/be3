use super::*;

#[test]
fn comptime_eval_evaluates_void_and_comptime_only_lines() {
    let mut env = new_env();
    let block = AnalysisBlock {
        lines: vec![
            AnalysisLine::Void { pos: pos_at(0) },
            AnalysisLine::ComptimeOnly { pos: pos_at(1) },
        ],
    };

    let results = comptime_eval(&mut env, &block).unwrap();

    assert_eq!(results.len(), 2);
    assert!(results[0].downcast_ref::<()>().is_some());
    assert!(results[1].downcast_ref::<()>().is_some());
}
