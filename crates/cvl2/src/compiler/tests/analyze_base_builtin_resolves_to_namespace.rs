use super::*;

#[test]
fn analyze_base_builtin_resolves_to_namespace() {
    let mut env = new_env();
    let mut block = AnalysisBlock { lines: Vec::new() };
    let node = builtin_ident("builtin", 0);

    let result = analyze_base(
        &mut env,
        ComptimeType::Unknown(ComptimeTypeUnknown { pos: pos_at(0) }),
        &node,
        &mut block,
    )
    .unwrap();

    assert!(matches!(result.ty, ComptimeType::Namespace(_)));
    assert_eq!(block.lines.len(), 1);
}
