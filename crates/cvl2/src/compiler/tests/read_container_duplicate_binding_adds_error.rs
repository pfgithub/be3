use super::*;

#[test]
fn read_container_duplicate_binding_adds_error() {
    let mut env = new_env();
    let line1 = op_seg(vec![def_binding("a", "x", 0)], 0);
    let sep1 = op_node(",", 5);
    let line2 = op_seg(vec![def_binding("a", "y", 10)], 10);
    let src = vec![binary_node(OpTag::Sep, vec![line1, sep1, line2], 0)];

    let _container = read_container(&mut env, pos_at(0), &src).unwrap();

    assert_eq!(env.errors.len(), 1);
    assert_eq!(env.errors[0].entries[0].message, "Duplicate binding name a");
    assert_eq!(env.scope.bindings.borrow().len(), 1);
    assert!(matches!(
        env.scope.bindings.borrow().get("a"),
        Some(Binding::Error { .. })
    ));
}
