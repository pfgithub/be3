use super::*;
use crate::{encode_frame, Hello, Modifiers, PluginIdentity, PointerButton, WheelUnit};

fn session() -> HostSession {
    HostSession::new("BE3", vec![Capability::Input, Capability::Lifecycle], true)
}

fn hello() -> Message {
    Message::Hello(Hello {
        minimum_version: PROTOCOL_VERSION,
        maximum_version: PROTOCOL_VERSION,
        plugin: PluginIdentity {
            id: "demo".into(),
            name: "Plugin Demo".into(),
            version: "1".into(),
        },
        capabilities: vec![Capability::Input],
    })
}

fn running_session() -> HostSession {
    let mut session = session();
    session.start(0);
    session.receive_frame(&encode_frame(&hello()).unwrap(), 1);
    session.next_outbound();
    session
}

fn screens(request_id: u64) -> Message {
    Message::Screens(crate::ScreenSet {
        request_id,
        screens: Vec::new(),
    })
}

fn input(event: InputEvent) -> Message {
    Message::Input(InputBatch {
        screen: crate::ScreenId(7),
        events: vec![event],
    })
}

mod a_superseded_request_is_forgotten;
mod coalesced_zoom_gestures_multiply;
mod disconnect_fails_the_session;
mod malformed_payload_fails_the_session;
mod queue_saturation_preserves_ordered_input;
mod repeated_start_and_shutdown_are_clean;
mod request_timeout_fails_the_session;
mod superseded_events_are_coalesced;
