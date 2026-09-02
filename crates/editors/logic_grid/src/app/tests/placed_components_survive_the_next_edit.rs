use super::*;
use logicgame::grid::ComponentPort;
use uuid::Uuid;

#[test]
fn placed_components_survive_the_next_edit() {
    let mut editor = LogicGridEditor::default();
    let kind = ComponentKind::subcomponent(
        Uuid::from_u128(7),
        logicgame::grid::Size::new(3, 2),
        vec![ComponentPort::input(
            0,
            Scale::ONE,
            ComponentSide::Left,
            0,
            1,
        )],
    )
    .unwrap();

    let id = editor.place(Point::new(4, 4), ComponentOrientation::Up, kind.clone());
    assert_eq!(
        editor
            .block
            .read()
            .and_then(|block| block.grid().component(id).cloned())
            .map(|component| component.kind),
        Some(kind)
    );

    editor.edit(LogicGridOperation::AddWire {
        wire: wire((0, 0), (2, 0), 1),
    });
    assert!(editor.grid.component(id).is_some());
}
