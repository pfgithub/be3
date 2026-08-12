use super::*;

#[test]
fn analyze_access_builtin_main_resolves_key() {
    let mut env = new_env();
    let mut block = AnalysisBlock { lines: Vec::new() };
    let node = builtin_ident("builtin", 0);
    let obj = analyze_base(
        &mut env,
        ComptimeType::Unknown(ComptimeTypeUnknown { pos: pos_at(0) }),
        &node,
        &mut block,
    )
    .unwrap();

    let key_idx = block_append(
        &mut block,
        AnalysisLine::ComptimeKey {
            pos: pos_at(1),
            narrow: ComptimeNarrowKey::String {
                key: "main".to_string(),
            },
        },
    );
    let prop = AnalysisResult {
        idx: key_idx,
        ty: ComptimeType::Key(ComptimeTypeKey {
            pos: pos_at(1),
            narrow: Some(ComptimeNarrowKey::String {
                key: "main".to_string(),
            }),
        }),
    };

    let result = analyze_access(
        &mut env,
        ComptimeType::Unknown(ComptimeTypeUnknown { pos: pos_at(0) }),
        obj,
        pos_at(2),
        prop,
        &mut block,
    )
    .unwrap();

    let ComptimeType::Key(key_ty) = result.ty else {
        panic!("expected key type");
    };
    let Some(ComptimeNarrowKey::Symbol { key, .. }) = key_ty.narrow else {
        panic!("expected symbol narrow");
    };
    assert_eq!(key, main_symbol());
}
