use super::*;

#[test]
fn widgets_are_inserted_at_their_location() {
    let mut builder = GuiBuilder::new();
    let group = insert(&mut builder, None, 0, container());
    let first = insert(&mut builder, Some(group), 0, label("First"));
    // An index past the end lands at the end instead of being dropped.
    let last = insert(&mut builder, Some(group), 99, label("Last"));
    let middle = insert(&mut builder, Some(group), 1, label("Middle"));

    let children = builder.children(Some(group)).unwrap();
    assert_eq!(
        children.iter().map(|child| child.id).collect::<Vec<_>>(),
        vec![first, middle, last]
    );
    assert_eq!(
        builder.location(middle),
        Some(GuiLocation::new(Some(group), 1))
    );
    assert_eq!(builder.location(group), Some(GuiLocation::new(None, 0)));
}
