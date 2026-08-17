use super::*;

fn hello() -> Message {
    Message::Hello(Hello {
        minimum_version: PROTOCOL_VERSION,
        maximum_version: PROTOCOL_VERSION,
        plugin: PluginIdentity {
            id: "demo".into(),
            name: "Plugin Demo".into(),
            version: "1.0".into(),
        },
        capabilities: vec![
            Capability::Input,
            Capability::Surface(SurfaceMechanism::WebExternalImage),
        ],
    })
}

mod frame_round_trips;
mod manifest_validation;
mod multiplexed_messages_round_trip;
mod rejects_collection_over_limit;
mod rejects_frame_over_limit;
mod rejects_malformed_payload;
mod rejects_oversized_block_payload;
mod rejects_truncated_frame;
mod rejects_unknown_message_kind;
