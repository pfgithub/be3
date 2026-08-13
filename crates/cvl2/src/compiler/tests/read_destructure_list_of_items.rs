use super::*;

#[test]
fn read_destructure_list_of_items() {
    let mut env = new_env();
    let comma_list = binary_node(
        OpTag::Sep,
        vec![
            op_seg(vec![normal_ident("a", 1)], 1),
            op_node(",", 2),
            op_seg(vec![normal_ident("b", 3)], 3),
        ],
        0,
    );
    let src = vec![list_block(vec![comma_list], 0)];
    let mut targets = Vec::new();

    let result = read_destructure(&mut env, pos_at(0), &src, &mut targets).unwrap();

    let DestructureExtract::List { items, .. } = result.extract else {
        panic!("expected list extract");
    };
    assert_eq!(items.len(), 2);
    let DestructureExtract::SingleItem { target: t0, .. } = &items[0] else {
        panic!("expected single_item extract");
    };
    let DestructureExtract::SingleItem { target: t1, .. } = &items[1] else {
        panic!("expected single_item extract");
    };
    assert_eq!(targets[*t0].name, "a");
    assert_eq!(targets[*t1].name, "b");

    let Type::Tuple(tuple) = result.ty else {
        panic!("expected tuple type");
    };
    assert_eq!(tuple.children.len(), 2);
}
