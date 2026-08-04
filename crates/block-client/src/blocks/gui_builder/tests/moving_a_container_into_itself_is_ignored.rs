use super::*;

#[test]
fn moving_a_container_into_itself_is_ignored() {
    let mut builder = GuiBuilder::new();
    let outer = insert(&mut builder, None, 0, container());
    let inner = insert(&mut builder, Some(outer), 0, container());
    let text = insert(&mut builder, Some(inner), 0, label("Nested"));

    for parent in [outer, inner] {
        GuiBuilder::apply_operation(
            &mut builder,
            &GuiBuilderOperation::Move {
                id: outer,
                location: GuiLocation::new(Some(parent), 0),
            },
        );
    }

    assert_eq!(builder.location(outer), Some(GuiLocation::new(None, 0)));
    assert_eq!(
        builder.location(inner),
        Some(GuiLocation::new(Some(outer), 0))
    );
    assert_eq!(
        builder.location(text),
        Some(GuiLocation::new(Some(inner), 0))
    );

    GuiBuilder::apply_operation(
        &mut builder,
        &GuiBuilderOperation::Move {
            id: text,
            location: GuiLocation::new(None, 0),
        },
    );
    assert_eq!(builder.location(text), Some(GuiLocation::new(None, 0)));
    assert_eq!(builder.location(outer), Some(GuiLocation::new(None, 1)));
}
