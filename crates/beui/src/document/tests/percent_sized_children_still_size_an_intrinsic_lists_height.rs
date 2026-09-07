use super::*;

#[test]
fn percent_sized_children_still_size_an_intrinsic_lists_height() {
    let mut document = Document::new();
    let left = document.create_padding(0.0, 20.0);
    let right = document.create_padding(0.0, 20.0);

    let row = document.create_list(Direction::Horizontal, 0.0);
    document.append_child(row, left, ItemSize::Percent(50.0));
    document.append_child(row, right, ItemSize::Percent(50.0));

    let outer = document.create_list(Direction::Vertical, 0.0);
    document.append_child(outer, row, ItemSize::Intrinsic);
    document.set_root(outer);

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(row).height(), 40.0);
}
