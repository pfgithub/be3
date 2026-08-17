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
    plugin: PluginIdentity,
}

impl Default for ClientSession {
    fn default() -> Self {
        Self::new("", "", "")
    }
}

impl ClientSession {
    pub fn new(id: &str, name: &str, version: &str) -> Self {
        Self {
            state: State::AwaitingHello,
            viewport_request_id: None,
            plugin: PluginIdentity {
                id: id.into(),
                name: name.into(),
                version: version.into(),
            },
        }
    }
    pub fn state(&self) -> State {
        self.state
    }

    pub fn hello(&self) -> Message {
        #[allow(unused_mut)]
        let mut capabilities = vec![Capability::Lifecycle, Capability::Input];
        #[cfg(target_os = "macos")]
        capabilities.push(Capability::Surface(
            block_plugin_api::SurfaceMechanism::MacOsIoSurface,
        ));
        #[cfg(target_os = "windows")]
        capabilities.push(Capability::Surface(
            block_plugin_api::SurfaceMechanism::WindowsDxgi,
        ));
        #[cfg(target_os = "linux")]
        capabilities.push(Capability::Surface(
            block_plugin_api::SurfaceMechanism::LinuxDmaBuf,
        ));
        #[cfg(target_arch = "wasm32")]
        capabilities.push(Capability::Surface(
            block_plugin_api::SurfaceMechanism::WebExternalImage,
        ));
        Message::Hello(Hello {
            minimum_version: PROTOCOL_VERSION,
            maximum_version: PROTOCOL_VERSION,
            plugin: self.plugin.clone(),
            capabilities,
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
