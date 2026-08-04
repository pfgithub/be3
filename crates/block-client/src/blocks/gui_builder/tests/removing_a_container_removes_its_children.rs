use super::*;

#[test]
fn removing_a_container_removes_its_children() {
    let mut builder = GuiBuilder::new();
    let outer = insert(&mut builder, None, 0, container());
    let inner = insert(&mut builder, Some(outer), 0, container());
    let nested = insert(&mut builder, Some(inner), 0, label("Nested"));
    let sibling = insert(&mut builder, None, 1, label("Sibling"));

    GuiBuilder::apply_operation(&mut builder, &GuiBuilderOperation::Remove { id: outer });
    // Removing something that is already gone is a no-op.
    GuiBuilder::apply_operation(&mut builder, &GuiBuilderOperation::Remove { id: nested });

    assert!(builder.widget(outer).is_none());
    assert!(builder.widget(inner).is_none());
    assert!(builder.widget(nested).is_none());
    assert_eq!(builder.location(sibling), Some(GuiLocation::new(None, 0)));
}
