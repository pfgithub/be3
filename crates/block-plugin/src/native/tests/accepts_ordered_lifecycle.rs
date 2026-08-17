use super::*;

#[test]
fn accepts_ordered_lifecycle() {
    let mut session = ClientSession::default();
    assert!(matches!(session.hello(), Message::Hello(_)));
    accept(&mut session);
    assert_eq!(session.state(), State::Running);

    let responses = session.receive(Message::CreateViewport(block_plugin_api::CreateViewport {
        request_id: 7,
        metrics: ViewportMetrics {
            logical_width: 100.0,
            logical_height: 100.0,
            pixel_width: 100,
            pixel_height: 100,
            scale_factor: 1.0,
        },
    }));
    assert_eq!(responses, vec![Message::Acknowledged { request_id: 7 }]);
    assert!(session
        .receive(Message::Input(InputBatch {
            viewport_request_id: 7,
            events: Vec::new(),
        }))
        .is_empty());
    assert_eq!(
        session.receive(Message::Shutdown),
        vec![Message::ShutdownAcknowledged]
    );
    assert_eq!(session.state(), State::Closed);
}
