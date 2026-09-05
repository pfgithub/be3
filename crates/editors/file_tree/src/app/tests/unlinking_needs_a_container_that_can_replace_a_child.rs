use super::*;

#[test]
fn unlinking_needs_a_container_that_can_replace_a_child() {
    let app = app();
    let host = EditorHost::default();
    let client = client();

    let refusing = catalog(ChildEdits::default());
    let frame = Frame {
        host: &host,
        client: &client,
        types: &refusing,
    };
    assert_eq!(
        app.unlink_permission(&frame, Some(CONTAINER)),
        Err("This container doesn't support replacing a reference")
    );
    assert_eq!(
        app.unlink_permission(&frame, None),
        Err("Loading\u{2026}"),
        "a row with no container is not a reference held anywhere"
    );

    let replacing = catalog(ChildEdits {
        add: false,
        delete: false,
        replace: true,
    });
    let frame = Frame {
        host: &host,
        client: &client,
        types: &replacing,
    };
    assert_eq!(app.unlink_permission(&frame, Some(CONTAINER)), Ok(()));
}
