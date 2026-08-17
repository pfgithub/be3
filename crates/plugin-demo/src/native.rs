use block_plugin_api::{
    Capability, ErrorCode, Hello, Message, PluginIdentity, ProtocolError, PROTOCOL_VERSION,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    AwaitingHello,
    Running,
    Closed,
    Failed,
}

pub struct ClientSession {
    state: State,
    viewport_request_id: Option<u64>,
}

impl Default for ClientSession {
    fn default() -> Self {
        Self {
            state: State::AwaitingHello,
            viewport_request_id: None,
        }
    }
}

impl ClientSession {
    pub fn state(&self) -> State {
        self.state
    }

    pub fn hello(&self) -> Message {
        Message::Hello(Hello {
            minimum_version: PROTOCOL_VERSION,
            maximum_version: PROTOCOL_VERSION,
            plugin: PluginIdentity {
                id: "be3.plugin-demo".into(),
                name: "Plugin Demo".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            capabilities: vec![Capability::Lifecycle, Capability::Input],
        })
    }

    pub fn receive(&mut self, message: Message) -> Vec<Message> {
        let result = match (&self.state, message) {
            (State::AwaitingHello, Message::HelloAccepted(accepted))
                if accepted.version == PROTOCOL_VERSION =>
            {
                self.state = State::Running;
                Ok(Vec::new())
            }
            (State::AwaitingHello, Message::HelloRejected(error)) => Err(error.message),
            (State::Running, Message::CreateViewport(viewport)) => {
                if self.viewport_request_id.is_some() {
                    Err("a viewport already exists".into())
                } else {
                    self.viewport_request_id = Some(viewport.request_id);
                    Ok(vec![Message::Acknowledged {
                        request_id: viewport.request_id,
                    }])
                }
            }
            (State::Running, Message::ResizeViewport(_)) if self.viewport_request_id.is_some() => {
                Ok(Vec::new())
            }
            (State::Running, Message::Input(input))
                if self.viewport_request_id == Some(input.viewport_request_id) =>
            {
                Ok(Vec::new())
            }
            (State::Running, Message::Ping { nonce }) => Ok(vec![Message::Pong { nonce }]),
            (State::Running, Message::Shutdown) => {
                self.state = State::Closed;
                Ok(vec![Message::ShutdownAcknowledged])
            }
            (State::Closed | State::Failed, _) => Ok(Vec::new()),
            _ => Err("host message is invalid in the current plugin state".into()),
        };

        match result {
            Ok(messages) => messages,
            Err(message) => {
                self.state = State::Failed;
                vec![Message::Error(ProtocolError {
                    request_id: None,
                    code: ErrorCode::InvalidState,
                    message,
                })]
            }
        }
    }
}

#[cfg(test)]
mod tests;
