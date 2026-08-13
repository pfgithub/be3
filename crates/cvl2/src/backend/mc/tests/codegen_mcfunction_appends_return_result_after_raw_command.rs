use super::*;

#[test]
fn codegen_mcfunction_appends_return_result_after_raw_command() {
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
        gid: 0,
        internal_ns: "ns".to_string(),
    };

    let result = codegen_mcfunction(
        &mut env,
        &mut ctx,
        &block,
        RuntimeValue::Comptime(ComptimeValue::McResult(ComptimeValueMcResult { result: 1 })),
    )
    .unwrap();

    assert_eq!(result, "say hi\nreturn 1");
}
