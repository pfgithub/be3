use super::*;

#[test]
fn codegen_mcfunction_errors_when_result_was_lost() {
    let validate = Symbol::new();
    let block = AnalysisBlock {
        offset: 0,
        validate,
        lines: vec![
            AnalysisLine::McExecRaw {
                pos: pos_at(0),
                command: RuntimeValue::Comptime(ComptimeValue::McNbtRef(
                    ComptimeValueMcNbtRef::String("say first".to_string()),
                )),
            },
            AnalysisLine::McExecRaw {
                pos: pos_at(1),
                command: RuntimeValue::Comptime(ComptimeValue::McNbtRef(
                    ComptimeValueMcNbtRef::String("say second".to_string()),
                )),
            },
        ],
    };
    let mut env = new_env();
    let mut ctx = McCodegenCtx {
        fns: HashMap::new(),
        gid: 0,
        internal_ns: "ns".to_string(),
    };

    let err = codegen_mcfunction(
        &mut env,
        &mut ctx,
        &block,
        RuntimeValue::Runtime(BlockIdx(0, validate)),
    )
    .unwrap_err();

    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };
    assert_eq!(e.entries[0].message, "this result was lost");
}
