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

    let result = read_destructure(&mut env, pos_at(0), &src).unwrap();

    let DestructureExtract::List { items, .. } = result.extract else {
        panic!("expected list extract");
    };
    assert_eq!(items.len(), 2);
    assert!(matches!(&items[0], DestructureExtract::SingleItem { name, .. } if name == "a"));
    assert!(matches!(&items[1], DestructureExtract::SingleItem { name, .. } if name == "b"));

    let ComptimeType::Tuple(tuple) = result.ty else {
        panic!("expected tuple type");
    };
    assert_eq!(tuple.children.len(), 2);
}
