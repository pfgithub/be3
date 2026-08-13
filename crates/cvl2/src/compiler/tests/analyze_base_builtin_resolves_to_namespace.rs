use super::*;

#[test]
fn analyze_base_builtin_resolves_to_namespace() {
    let mut env = new_env();
    let mut block = empty_block();
    let node = builtin_ident("builtin", 0);

    let result = analyze_base(&mut env, Type::Unknown(TypeUnknown), &node, &mut block).unwrap();

    assert!(matches!(result.ty, Type::CtNamespace(_)));
    assert!(block.lines.is_empty());
}
