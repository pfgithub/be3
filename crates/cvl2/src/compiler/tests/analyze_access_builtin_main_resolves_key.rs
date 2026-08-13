use super::*;

#[test]
fn analyze_access_builtin_main_resolves_key() {
    let mut env = new_env();
    let mut block = empty_block();
    let node = builtin_ident("builtin", 0);
    let obj = analyze_base(&mut env, Type::Unknown(TypeUnknown), &node, &mut block).unwrap();

    let prop = AnalysisResult {
        ty: Type::CtKey(CtKey),
        value: RuntimeValue::Comptime(ComptimeValue::Key(ComptimeValueKey::String {
            key: "build".to_string(),
        })),
    };

    let result = analyze_access(
        &mut env,
        Type::Unknown(TypeUnknown),
        obj,
        pos_at(2),
        prop,
        &mut block,
    )
    .unwrap();

    let RuntimeValue::Comptime(ComptimeValue::Key(ComptimeValueKey::Symbol { key, .. })) =
        result.value
    else {
        panic!("expected symbol key");
    };
    assert_eq!(key, build_symbol());
}
