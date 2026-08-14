use super::*;

#[test]
fn codegen_mcfunction_errors_on_unsupported_line_kind() {
    let validate = Symbol::new();
    let block = AnalysisBlock {
        offset: 0,
        validate,
        lines: vec![AnalysisLine::Break {
            pos: pos_at(0),
            value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
        }],
    };
    let mut env = new_env();
    let mut ctx = McCodegenCtx {
        fns: HashMap::new(),
        fn_order: Vec::new(),
        gid: 0,
        internal_ns: "ns".to_string(),
    };

    let err = codegen_mcfunction(
        &mut env,
        &mut ctx,
        &block,
        RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
    )
    .unwrap_err();

    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };
    assert!(e.entries[0]
        .message
        .starts_with("TODO codegenMcfunction line:"));
}
