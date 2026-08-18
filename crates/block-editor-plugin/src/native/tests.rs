use super::*;
use block_plugin_api::{HelloAccepted, InputBatch, ViewportMetrics};

fn accept(session: &mut ClientSession) {
    session.receive(Message::HelloAccepted(HelloAccepted {
        version: PROTOCOL_VERSION,
        host_name: "test host".into(),
        capabilities: vec![Capability::Lifecycle, Capability::Input],
    }));
}

mod accepts_ordered_lifecycle;
mod opens_and_closes_editor_instance;
mod rejects_out_of_order_messages;
