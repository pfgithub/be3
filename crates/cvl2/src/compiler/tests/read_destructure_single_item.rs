use super::*;

#[test]
fn read_destructure_single_item() {
    let mut env = new_env();
    let src = vec![normal_ident("a", 0)];
    let mut targets = Vec::new();
    let result = read_destructure(&mut env, pos_at(0), &src, &mut targets).unwrap();
    match result.extract {
        DestructureExtract::SingleItem { target, .. } => assert_eq!(targets[target].name, "a"),
        _ => panic!("expected single_item extract"),
    }
    assert!(matches!(result.ty, Type::Unknown(_)));
}
