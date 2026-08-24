use super::*;

#[test]
fn branch_generation_order_is_irrelevant() {
    let mut first = demo_world(11);
    let mut second = demo_world(11);
    let root = NodeId::root();
    first.expand(&root);
    second.expand(&root);
    let a = root.child(0);
    let b = root.child(1);

    first.expand(&a);
    first.expand(&b);
    second.expand(&b);
    second.expand(&a);

    for id in [a.child(0), a.child(1), b.child(0), b.child(1)] {
        assert_eq!(bounds(&first, &id), bounds(&second, &id));
    }
}
