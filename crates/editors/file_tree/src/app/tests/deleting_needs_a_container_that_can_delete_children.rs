use super::*;

#[test]
fn deleting_needs_a_container_that_can_delete_children() {
    let app = app();
    let host = EditorHost::default();
    let client = client();

    let refusing = catalog(ChildEdits::default());
    let frame = Frame {
        host: &host,
        client: &client,
        types: &refusing,
    };
    assert!(!app.can_move_out_of(&frame, BlockSource::Block(CONTAINER), LISTED, false));
    assert!(
        app.can_move_out_of(&frame, BlockSource::Root, LISTED, false),
        "a root block is always listed by the root list itself"
    );
    assert!(app.can_move_out_of(&frame, BlockSource::Orphaned, LISTED, true));

    let deleting = catalog(ChildEdits {
        add: false,
        delete: true,
        replace: false,
    });
    let frame = Frame {
        host: &host,
        client: &client,
        types: &deleting,
    };
    assert!(app.can_move_out_of(&frame, BlockSource::Block(CONTAINER), LISTED, false));
}
