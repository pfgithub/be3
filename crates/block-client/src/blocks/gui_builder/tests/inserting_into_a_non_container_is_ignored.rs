use super::*;

#[test]
fn inserting_into_a_non_container_is_ignored() {
    let mut builder = GuiBuilder::new();
    let text = insert(&mut builder, None, 0, label("Not a container"));
    let missing = Uuid::new_v4();

    let child = insert(&mut builder, Some(text), 0, label("Child"));
    let orphan = insert(&mut builder, Some(missing), 0, label("Orphan"));

    assert!(builder.widget(child).is_none());
    assert!(builder.widget(orphan).is_none());
    assert!(builder.children(Some(text)).is_none());
    assert_eq!(builder.widgets().len(), 1);
}
