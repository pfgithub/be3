use super::{editor, entities, entity, BlockRef, CanvasEntityKind, Uuid};
use block_editor_plugin::App as _;

#[test]
fn replacing_a_referenced_block_rewrites_the_entity() {
    let old = Uuid::from_u128(0x0100_0000_0000_4000_8000_0000_0000_0011);
    let new = Uuid::from_u128(0x0200_0000_0000_4000_8000_0000_0000_0012);
    let mut referencing = entity(Uuid::from_u128(3));
    referencing.kind = CanvasEntityKind::DirectEditor {
        block_id: BlockRef::Direct(old),
        scale: 1.0,
    };
    let (mut editor, block) = editor(std::slice::from_ref(&referencing));

    assert!(editor.app().replace_child(old, new));

    let kinds = entities(&block)
        .into_iter()
        .map(|entity| entity.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [CanvasEntityKind::DirectEditor {
            block_id: BlockRef::Direct(new),
            scale: 1.0,
        }]
    );
}
