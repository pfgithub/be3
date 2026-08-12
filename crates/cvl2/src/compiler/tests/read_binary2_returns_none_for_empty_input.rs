use super::*;

#[test]
fn read_binary2_returns_none_for_empty_input() {
    let mut env = new_env();
    let result = read_binary2(&mut env, pos_at(0), &[], OpTag::Def).unwrap();
    assert!(result.is_none());
}
