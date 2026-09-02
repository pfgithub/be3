use super::{block_url, editor, text, Uuid, WORKSPACE_ID};
use block_editor_plugin::App as _;

#[test]
fn replacing_a_referenced_block_rewrites_its_url() {
    let old = Uuid::from_u128(0x01d0_0000_0000_4000_8000_0000_0000_0001);
    let new = Uuid::from_u128(0x0e00_0000_0000_4000_8000_0000_0000_0002);
    let (mut editor, block) = editor(&format!("see {}\n", block_url(WORKSPACE_ID, old)));

    assert!(editor.app().replace_child(old, new));

    assert_eq!(
        text(&block),
        format!("see {}\n", block_url(WORKSPACE_ID, new))
    );
}
