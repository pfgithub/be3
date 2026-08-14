use super::*;

#[test]
fn codegen_mcfunction_emits_raw_command_as_return_run() {
    let validate = Symbol::new();
    let block = AnalysisBlock {
        offset: 0,
        validate,
        lines: vec![AnalysisLine::McExecRaw {
            pos: pos_at(0),
            command: RuntimeValue::Comptime(ComptimeValue::McNbtRef(
                ComptimeValueMcNbtRef::String("say hi".to_string()),
            )),
        }],
    };
    let mut env = new_env();
    let mut ctx = McCodegenCtx {
        fns: HashMap::new(),
        fn_order: Vec::new(),
        gid: 0,
        internal_ns: "ns".to_string(),
    };

    let result = codegen_mcfunction(
        &mut env,
        &mut ctx,
        &block,
        RuntimeValue::Runtime(BlockIdx(0, validate)),
    )
    .unwrap();

    assert_eq!(result, "return run say hi");
}
