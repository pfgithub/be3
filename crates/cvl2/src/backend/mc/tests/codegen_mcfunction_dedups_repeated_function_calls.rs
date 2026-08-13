use super::*;

#[test]
fn codegen_mcfunction_dedups_repeated_function_calls() {
    let validate = Symbol::new();
    let mut env = new_env();
    let target = dummy_fn(&env, pos_at(0));
    let block = AnalysisBlock {
        offset: 0,
        validate,
        lines: vec![
            AnalysisLine::Call {
                pos: pos_at(1),
                method: RuntimeValue::Comptime(ComptimeValue::Fn(target.clone())),
                arg: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
            },
            AnalysisLine::Call {
                pos: pos_at(2),
                method: RuntimeValue::Comptime(ComptimeValue::Fn(target.clone())),
                arg: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
            },
        ],
    };
    let mut ctx = McCodegenCtx {
        fns: HashMap::new(),
        gid: 0,
        internal_ns: "ns".to_string(),
    };

    let result = codegen_mcfunction(
        &mut env,
        &mut ctx,
        &block,
        RuntimeValue::Runtime(BlockIdx(1, validate)),
    )
    .unwrap();

    assert_eq!(result, "function ns:_0\nreturn run function ns:_0");
    assert_eq!(ctx.gid, 1);
    assert_eq!(ctx.fns.len(), 1);
}
