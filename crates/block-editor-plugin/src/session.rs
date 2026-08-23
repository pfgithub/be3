use block_plugin_api::{
    Capability, EditorInstanceId, ErrorCode, Hello, Message, PluginIdentity, ProtocolError,
    ScreenId, PROTOCOL_VERSION,
};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    AwaitingHello,
    Running,
    Closed,
    Failed,
}

pub struct ClientSession {
    state: State,
    screens: HashSet<ScreenId>,
    plugin: PluginIdentity,
    instances: HashSet<EditorInstanceId>,
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
            screens: HashSet::new(),
            plugin: PluginIdentity {
                id: id.into(),
                name: name.into(),
                version: version.into(),
            },
            instances: HashSet::new(),
        }
    }
    pub fn state(&self) -> State {
        self.state
    }

    pub fn hello(&self) -> Message {
        #[allow(unused_mut)]
        let mut capabilities = vec![Capability::Lifecycle, Capability::Input];
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
            (State::Running, Message::Screens(set)) => {
                if set
                    .screens
                    .iter()
                    .any(|screen| !self.instances.contains(&screen.instance))
                {
                    Err("a screen referenced an unopened editor instance".into())
                } else {
                    self.screens = set.screens.iter().map(|screen| screen.screen).collect();
                    Ok(vec![Message::Acknowledged {
                        request_id: set.request_id,
                    }])
                }
            }
            (State::Running, Message::Input(input)) if self.screens.contains(&input.screen) => {
                Ok(Vec::new())
            }
            (
                State::Running,
                Message::Editor(
                    block_plugin_api::EditorMessage::Open { instance, .. }
                    | block_plugin_api::EditorMessage::OpenCreation { instance, .. }
                    | block_plugin_api::EditorMessage::OpenArtifact { instance, .. },
                ),
            ) if !self.instances.contains(&instance) => {
                self.instances.insert(instance);
                Ok(vec![Message::Editor(
                    block_plugin_api::EditorMessage::Acknowledged {
                        instance,
                        request_id: 0,
                    },
                )])
            }
            (
                State::Running,
                Message::Editor(
                    block_plugin_api::EditorMessage::EditabilityChanged { instance, .. }
                    | block_plugin_api::EditorMessage::ViewChanged { instance, .. },
                ),
            ) if self.instances.contains(&instance) => Ok(Vec::new()),
            (
                State::Running,
                Message::Editor(block_plugin_api::EditorMessage::Close { instance }),
            ) if self.instances.contains(&instance) => {
                self.instances.remove(&instance);
                Ok(vec![Message::Editor(
                    block_plugin_api::EditorMessage::Acknowledged {
                        instance,
                        request_id: 0,
                    },
                )])
            }
            (State::Running, Message::Client(_)) if !self.instances.is_empty() => Ok(Vec::new()),
            (State::Running, Message::BlockTypes(_)) => Ok(Vec::new()),
            (
                State::Running,
                Message::Editor(
                    block_plugin_api::EditorMessage::DragOver { instance, .. }
                    | block_plugin_api::EditorMessage::DragLeft { instance }
                    | block_plugin_api::EditorMessage::CommitCreation { instance }
                    | block_plugin_api::EditorMessage::FilePicked { instance, .. }
                    | block_plugin_api::EditorMessage::ArtifactSettings { instance, .. }
                    | block_plugin_api::EditorMessage::RegenerateArtifact { instance, .. },
                ),
            ) if self.instances.contains(&instance) => Ok(Vec::new()),
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
