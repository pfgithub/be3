use crate::{
    decode_frame, Capability, DecodeError, ErrorCode, HelloAccepted, InputBatch, InputEvent,
    Message, ProtocolError, MAX_QUEUED_MESSAGES, PROTOCOL_VERSION, REQUEST_TIMEOUT_MILLISECONDS,
};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Starting,
    Running,
    ShuttingDown,
    Closed,
    Failed(SessionFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionFailure {
    MalformedMessage,
    Protocol(String),
    RequestTimedOut(u64),
    StartupTimedOut,
    ShutdownTimedOut,
    Disconnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueError {
    NotRunning,
    Full,
    DuplicateRequest,
}

pub struct HostSession {
    state: SessionState,
    host_name: String,
    capabilities: Vec<Capability>,
    queue: VecDeque<Message>,
    requests: HashMap<u64, u64>,
    lifecycle_deadline: Option<u64>,
}

impl HostSession {
    pub fn new(host_name: impl Into<String>, capabilities: Vec<Capability>) -> Self {
        Self {
            state: SessionState::Idle,
            host_name: host_name.into(),
            capabilities,
            queue: VecDeque::new(),
            requests: HashMap::new(),
            lifecycle_deadline: None,
        }
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn queued_message_count(&self) -> usize {
        self.queue.len()
    }

    pub fn pending_request_count(&self) -> usize {
        self.requests.len()
    }

    pub fn start(&mut self, now_milliseconds: u64) {
        self.queue.clear();
        self.requests.clear();
        self.state = SessionState::Starting;
        self.lifecycle_deadline = Some(now_milliseconds + REQUEST_TIMEOUT_MILLISECONDS);
    }

    pub fn receive_frame(&mut self, frame: &[u8], now_milliseconds: u64) {
        match decode_frame(frame) {
            Ok(message) => self.receive(message, now_milliseconds),
            Err(error) => self.fail(decode_failure(error)),
        }
    }

    pub fn receive(&mut self, message: Message, now_milliseconds: u64) {
        match (&self.state, message) {
            (SessionState::Starting, Message::Hello(hello)) => {
                if hello.minimum_version > PROTOCOL_VERSION
                    || hello.maximum_version < PROTOCOL_VERSION
                {
                    let error = ProtocolError {
                        request_id: None,
                        code: ErrorCode::UnsupportedVersion,
                        message: "no compatible protocol version".into(),
                    };
                    self.queue.push_back(Message::HelloRejected(error));
                    self.fail(SessionFailure::Protocol(
                        "no compatible protocol version".into(),
                    ));
                    return;
                }
                self.queue.push_back(Message::HelloAccepted(HelloAccepted {
                    version: PROTOCOL_VERSION,
                    host_name: self.host_name.clone(),
                    capabilities: self.capabilities.clone(),
                }));
                self.state = SessionState::Running;
                self.lifecycle_deadline = None;
            }
            (SessionState::Running, Message::Acknowledged { request_id }) => {
                if self.requests.remove(&request_id).is_none() {
                    self.fail(SessionFailure::Protocol(format!(
                        "acknowledgement for unknown request {request_id}"
                    )));
                }
            }
            (SessionState::Running, Message::Error(error)) => {
                if let Some(request_id) = error.request_id {
                    self.requests.remove(&request_id);
                }
                self.fail(SessionFailure::Protocol(error.message));
            }
            (SessionState::ShuttingDown, Message::ShutdownAcknowledged) => {
                self.state = SessionState::Closed;
                self.lifecycle_deadline = None;
                self.requests.clear();
            }
            (SessionState::Failed(_), _) | (SessionState::Closed, _) => {}
            _ => self.fail(SessionFailure::Protocol(
                "message is invalid in the current session state".into(),
            )),
        }
        self.tick(now_milliseconds);
    }

    pub fn enqueue(&mut self, message: Message) -> Result<(), QueueError> {
        if self.state != SessionState::Running {
            return Err(QueueError::NotRunning);
        }
        if coalesce(&mut self.queue, &message) {
            return Ok(());
        }
        if self.queue.len() == MAX_QUEUED_MESSAGES {
            return Err(QueueError::Full);
        }
        self.queue.push_back(message);
        Ok(())
    }

    pub fn enqueue_request(
        &mut self,
        request_id: u64,
        message: Message,
        now_milliseconds: u64,
    ) -> Result<(), QueueError> {
        if self.requests.contains_key(&request_id) {
            return Err(QueueError::DuplicateRequest);
        }
        self.enqueue(message)?;
        self.requests
            .insert(request_id, now_milliseconds + REQUEST_TIMEOUT_MILLISECONDS);
        Ok(())
    }

    pub fn next_outbound(&mut self) -> Option<Message> {
        self.queue.pop_front()
    }

    pub fn shutdown(&mut self, now_milliseconds: u64) {
        match self.state {
            SessionState::Idle | SessionState::Closed | SessionState::Failed(_) => {
                self.state = SessionState::Closed;
                self.lifecycle_deadline = None;
            }
            SessionState::Starting | SessionState::Running => {
                self.queue.clear();
                self.queue.push_back(Message::Shutdown);
                self.state = SessionState::ShuttingDown;
                self.lifecycle_deadline = Some(now_milliseconds + REQUEST_TIMEOUT_MILLISECONDS);
            }
            SessionState::ShuttingDown => {}
        }
    }

    pub fn disconnected(&mut self) {
        if !matches!(self.state, SessionState::Closed | SessionState::Failed(_)) {
            self.fail(SessionFailure::Disconnected);
        }
    }

    pub fn tick(&mut self, now_milliseconds: u64) {
        if let Some((&request_id, _)) = self
            .requests
            .iter()
            .find(|(_, deadline)| now_milliseconds >= **deadline)
        {
            self.fail(SessionFailure::RequestTimedOut(request_id));
            return;
        }
        if self
            .lifecycle_deadline
            .is_some_and(|deadline| now_milliseconds >= deadline)
        {
            let failure = match self.state {
                SessionState::Starting => SessionFailure::StartupTimedOut,
                SessionState::ShuttingDown => SessionFailure::ShutdownTimedOut,
                _ => return,
            };
            self.fail(failure);
        }
    }

    fn fail(&mut self, failure: SessionFailure) {
        self.state = SessionState::Failed(failure);
        self.lifecycle_deadline = None;
        self.requests.clear();
    }
}

fn decode_failure(error: DecodeError) -> SessionFailure {
    match error {
        DecodeError::MalformedPayload
        | DecodeError::FrameTooLarge { .. }
        | DecodeError::TruncatedFrame { .. }
        | DecodeError::LimitExceeded(_) => SessionFailure::MalformedMessage,
    }
}

fn coalesce(queue: &mut VecDeque<Message>, incoming: &Message) -> bool {
    let Some(last) = queue.back_mut() else {
        return false;
    };
    match (last, incoming) {
        (Message::Screens(current), Message::Screens(incoming)) => {
            *current = incoming.clone();
            true
        }
        (Message::Input(current), Message::Input(incoming))
            if current.screen == incoming.screen
                && current.events.len() == 1
                && incoming.events.len() == 1 =>
        {
            coalesce_input(&mut current.events[0], &incoming.events[0])
        }
        _ => false,
    }
}

fn coalesce_input(current: &mut InputEvent, incoming: &InputEvent) -> bool {
    match (current, incoming) {
        (InputEvent::PointerMoved { x, y }, InputEvent::PointerMoved { x: new_x, y: new_y }) => {
            *x = *new_x;
            *y = *new_y;
            true
        }
        (
            InputEvent::Wheel { x, y, unit },
            InputEvent::Wheel {
                x: new_x,
                y: new_y,
                unit: new_unit,
            },
        ) if unit == new_unit => {
            *x += new_x;
            *y += new_y;
            true
        }
        (InputEvent::Modifiers(current), InputEvent::Modifiers(incoming))
            if current == incoming =>
        {
            true
        }
        _ => false,
    }
}

impl From<InputBatch> for Message {
    fn from(input: InputBatch) -> Self {
        Self::Input(input)
    }
}

#[cfg(test)]
mod tests;
