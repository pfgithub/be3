use super::*;
use block_plugin_api::{ChildId, ChildStatus};

fn status(instance: EditorInstanceId) -> ChildStatus {
    ChildStatus {
        instance,
        region: EditorRegion::Frame,
        child: ChildId(1),
        available: true,
        intrinsic_width: 320.0,
        intrinsic_height: 180.0,
        aspect_ratio: 0.0,
        hovered: false,
        active: false,
        error: None,
    }
}

#[test]
fn rejects_child_statuses_for_unopened_instances() {
    let mut session = ClientSession::default();
    accept(&mut session);
    let instance = EditorInstanceId(1);
    open(&mut session, instance);
    assert!(session
        .receive(Message::ChildStatuses(vec![status(instance)]))
        .is_empty());
    assert_eq!(session.state(), State::Running);

    let responses = session.receive(Message::ChildStatuses(vec![status(EditorInstanceId(9))]));
    assert!(matches!(responses.as_slice(), [Message::Error(_)]));
    assert_eq!(session.state(), State::Failed);
}
