use super::*;

#[test]
fn opens_and_closes_editor_instance() {
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    accept(&mut session);
    let instance = EditorInstanceId(4);
    let opened = session.receive(Message::Editor(block_plugin_api::EditorMessage::Open {
        instance,
        block_id: [1; 16],
        block_type: [2; 16],
        account_id: [3; 16],
        workspace_id: [4; 16],
        editable: true,
    }));
    assert_eq!(
        opened,
        vec![Message::Editor(
            block_plugin_api::EditorMessage::Acknowledged {
                instance,
                request_id: 0,
            }
        )]
    );
    let closed = session.receive(Message::Editor(block_plugin_api::EditorMessage::Close {
        instance,
    }));
    assert_eq!(
        closed,
        vec![Message::Editor(
            block_plugin_api::EditorMessage::Acknowledged {
                instance,
                request_id: 0,
            }
        )]
    );
    assert_eq!(session.state(), State::Running);
}
