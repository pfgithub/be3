use super::*;

#[test]
fn read_container_collects_bindings_and_lines() {
    let mut env = new_env();
    let line1 = op_seg(vec![def_binding("a", "x", 0)], 0);
    let sep1 = op_node(",", 5);
    let line2 = op_seg(vec![def_binding("b", "y", 10)], 10);
    let sep2 = op_node(",", 15);
    let line3 = op_seg(vec![normal_ident("c", 20)], 20);
    let src = vec![binary_node(
        OpTag::Sep,
        vec![line1, sep1, line2, sep2, line3],
        0,
    )];

    let container = read_container(&mut env, pos_at(0), &src).unwrap();

    assert!(env.errors.is_empty());
    assert_eq!(env.scope.bindings.borrow().len(), 2);
    let bindings = env.scope.bindings.borrow();
    let Some(Binding::Valid { decl: a_decl, .. }) = bindings.get("a") else {
        panic!("expected binding a");
    };
    assert!(matches!(&a_decl.ast().ast[0], SyntaxNode::Identifier(id) if id.str == "x"));
    let Some(Binding::Valid { decl: b_decl, .. }) = bindings.get("b") else {
        panic!("expected binding b");
    };
    assert!(matches!(&b_decl.ast().ast[0], SyntaxNode::Identifier(id) if id.str == "y"));
    assert_eq!(container.lines.len(), 1);
    assert!(matches!(&container.lines[0].items[0], SyntaxNode::Identifier(id) if id.str == "c"));
}
