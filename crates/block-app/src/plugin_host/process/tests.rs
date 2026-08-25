use super::*;
use block_plugin_api::{
    Hello, InputBatch, InputEvent, PluginIdentity, ScreenId, MAX_QUEUED_MESSAGES, PROTOCOL_VERSION,
};

#[derive(Default)]
struct Writer {
    messages: usize,
}

impl Writing for Writer {
    fn write(&mut self, _message: &Message) -> Result<(), CarrierError> {
        self.messages += 1;
        Ok(())
    }
}

fn running_session() -> HostSession {
    let mut session = HostSession::new("BE3", vec![Capability::Input], true);
    session.start(0);
    session.receive(
        Message::Hello(Hello {
            minimum_version: PROTOCOL_VERSION,
            maximum_version: PROTOCOL_VERSION,
            plugin: PluginIdentity {
                id: "be3.test".into(),
                name: "Test".into(),
                version: "1".into(),
            },
            capabilities: vec![Capability::Input],
        }),
        1,
    );
    while session.next_outbound().is_some() {}
    session
}

mod outbound_transport_does_not_saturate_the_session_queue;
