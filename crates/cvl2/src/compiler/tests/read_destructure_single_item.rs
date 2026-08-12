use super::*;

#[test]
fn read_destructure_single_item() {
    let mut env = new_env();
    let src = vec![normal_ident("a", 0)];
    let result = read_destructure(&mut env, pos_at(0), &src).unwrap();
    match result.extract {
        DestructureExtract::SingleItem { name, .. } => assert_eq!(name, "a"),
        _ => panic!("expected single_item extract"),
    }
    assert!(matches!(result.ty, ComptimeType::Unknown(_)));
}
