use super::*;

#[test]
#[should_panic(expected = "unreachable")]
fn analyze_namespace_bind_panics_on_non_key_result() {
    let mut env = new_env();
    let mut block = AnalysisBlock { lines: Vec::new() };
    let lhs = OperatorSegmentToken {
        pos: pos_at(0),
        items: vec![builtin_ident("builtin", 0)],
    };
    let op = OperatorToken {
        pos: pos_at(1),
        op: "::".to_string(),
    };
    let rhs = OperatorSegmentToken {
        pos: pos_at(2),
        items: vec![normal_ident("x", 2)],
    };

    let _ = analyze_namespace_bind(&mut env, (lhs, op, rhs), &mut block, BlockIdx(0));
}
