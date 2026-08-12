use super::*;

#[test]
fn analyze_call_not_supported_call_type_errors() {
    let mut env = new_env();
    let mut block = AnalysisBlock { lines: Vec::new() };
    let void_idx = block_append(&mut block, AnalysisLine::Void { pos: pos_at(0) });
    let method = AnalysisResult {
        idx: void_idx,
        ty: ComptimeType::Void(ComptimeTypeVoid { pos: pos_at(0) }),
    };

    let result = analyze_call(
        &mut env,
        ComptimeType::Void(ComptimeTypeVoid { pos: pos_at(0) }),
        pos_at(0),
        method,
        &mut |_env, _slot, _pos, block| AnalysisResult {
            idx: block_append(block, AnalysisLine::Void { pos: pos_at(0) }),
            ty: ComptimeType::Void(ComptimeTypeVoid { pos: pos_at(0) }),
        },
        &mut block,
    );
    let Err(err) = result else {
        panic!("expected an error");
    };

    assert_eq!(err.e.entries[0].message, "not supported call type: void");
}
