use super::*;

#[test]
fn read_binary_returns_empty_for_empty_input() {
    let mut env = new_env();
    let result = read_binary(&mut env, pos_at(0), &[], OpTag::Sep).unwrap();
    assert!(result.is_empty());
}
