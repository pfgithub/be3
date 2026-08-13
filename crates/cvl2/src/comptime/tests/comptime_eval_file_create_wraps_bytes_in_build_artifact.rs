use super::*;

#[test]
fn comptime_eval_file_create_wraps_bytes_in_build_artifact() {
    let validate = Symbol::new();
    let block = AnalysisBlock {
        offset: 0,
        validate,
        lines: vec![AnalysisLine::ComptimeFileCreate {
            pos: pos_at(0),
            value: RuntimeValue::Comptime(ComptimeValue::Uint8Array(ComptimeValueUint8Array {
                value: b"hello".to_vec(),
                sourcemap: Vec::new(),
            })),
        }],
    };
    let mut env = new_env();

    let result = comptime_eval(
        &mut env,
        &block,
        RuntimeValue::Runtime(BlockIdx(0, validate)),
        pos_at(1),
    )
    .unwrap();

    let ComptimeValue::BuildArtifact(ComptimeValueBuildArtifact::File(file)) = result else {
        panic!("expected a file build artifact");
    };
    assert_eq!(file.value, b"hello");
}
