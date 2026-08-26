use block_plugin_api::{
    desktop_attachments::CarrierError, encode_frame, Capability, HostSession, Message,
    SessionFailure, SessionState,
};
use std::{
    io::{self, BufRead, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread,
    time::{Duration, Instant},
};

use super::platform;

pub(super) type Attachment = platform::Attachment;

pub(super) enum SurfaceEvent {
    Surface(block_plugin_api::SurfaceDescriptor, Vec<Attachment>),
    Frame(block_plugin_api::FrameReady),
}

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

const TICK: Duration = Duration::from_millis(250);

pub(super) enum Received {
    Message(Message),
    Surface(SurfaceEvent),
}

enum Event {
    Send(Message),
    Received(Message, Vec<Attachment>),
    Shutdown,
    Ended(String),
}

enum Waited {
    Event(Event),
    Idle,
    Stopped,
}

pub(super) trait Reading {
    fn read(&mut self) -> Result<(Message, Vec<Attachment>), CarrierError>;
}

pub(super) trait Writing {
    fn write(&mut self, message: &Message) -> Result<(), CarrierError>;
}

struct Inbound {
    received: Sender<Received>,
    repaint: eframe::egui::Context,
}

pub(super) struct Process {
    events: Sender<Event>,
    exit: Receiver<String>,
    received: Receiver<Received>,
}

impl Process {
    pub(super) fn launch(executable: PathBuf, repaint: eframe::egui::Context) -> Self {
        let (events, event_receiver) = mpsc::channel();
        let (exit_sender, exit) = mpsc::channel();
        let (received_sender, received) = mpsc::channel();
        let event_sender = events.clone();
        thread::spawn(move || {
            let inbound = Inbound {
                received: received_sender,
                repaint,
            };
            let reason = match run(executable, event_sender, &event_receiver, inbound) {
                Ok(()) => "plugin exited".to_owned(),
                Err(error) => {
                    eprintln!("plugin host process failed: {error}");
                    error.to_string()
                }
            };
            exit_sender.send(reason).ok();
        });
        Self {
            events,
            exit,
            received,
        }
    }

    pub(super) fn shutdown(&mut self) {
        self.events.send(Event::Shutdown).ok();
    }

    pub(super) fn take_exit(&self) -> Option<String> {
        self.exit.try_recv().ok()
    }

    pub(super) fn send(&self, messages: Vec<Message>) {
        for message in messages {
            self.events.send(Event::Send(message)).ok();
        }
    }

    pub(super) fn receive(&self) -> Vec<Received> {
        self.received.try_iter().collect()
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run(
    executable: PathBuf,
    sender: Sender<Event>,
    events: &Receiver<Event>,
    inbound: Inbound,
) -> io::Result<()> {
    let endpoint = platform::Endpoint::create()?;
    let argument = endpoint.argument();
    let mut command = Command::new(&executable);
    command
        .args(["--endpoint", &argument])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    platform::prepare(&mut command, &executable);
    let mut child = command.spawn().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to launch {}: {error}", executable.display()),
        )
    })?;
    if let Some(stderr) = child.stderr.take() {
        thread::spawn(move || {
            for line in io::BufReader::new(stderr).lines().map_while(Result::ok) {
                eprintln!("plugin process: {line}");
            }
        });
    }
    let result = endpoint
        .accept(&child)
        .and_then(|connection| drive(connection, &mut child, sender, events, inbound));
    terminate(&mut child);
    result
}

fn drive(
    mut connection: platform::Connection,
    child: &mut Child,
    sender: Sender<Event>,
    events: &Receiver<Event>,
    inbound: Inbound,
) -> io::Result<()> {
    let started = Instant::now();
    let dark_theme = inbound.repaint.global_style().visuals.dark_mode;
    let mut session = handshake(&mut connection, started, dark_theme)?;
    let (reader, writer) = connection.split(child)?;
    thread::spawn(move || read_from_plugin(reader, sender));
    pump(writer, child, events, inbound, &mut session, started)
}

fn handshake(
    connection: &mut platform::Connection,
    started: Instant,
    dark_theme: bool,
) -> io::Result<HostSession> {
    let mut session = HostSession::new(
        "BE3",
        vec![
            Capability::Lifecycle,
            Capability::Input,
            Capability::Surface(platform::SURFACE_MECHANISM),
        ],
        dark_theme,
    );
    session.start(elapsed(started));
    let hello = read_message(connection.reader())?;
    session.receive(hello, elapsed(started));
    let writer = connection.writer();
    while let Some(message) = session.next_outbound() {
        writer.write_all(&encode(&message)?)?;
    }
    writer.flush()?;
    match session.state() {
        SessionState::Running => Ok(session),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "plugin handshake was rejected",
        )),
    }
}

fn read_from_plugin(mut carrier: impl Reading, events: Sender<Event>) {
    loop {
        match carrier.read() {
            Ok((message, attachments)) => {
                if events.send(Event::Received(message, attachments)).is_err() {
                    return;
                }
            }
            Err(error) => {
                events.send(Event::Ended(error.to_string())).ok();
                return;
            }
        }
    }
}

fn pump(
    mut carrier: impl Writing,
    child: &mut Child,
    events: &Receiver<Event>,
    inbound: Inbound,
    session: &mut HostSession,
    started: Instant,
) -> io::Result<()> {
    let mut disconnection = None;
    loop {
        let first = match wait(events, session) {
            Waited::Event(event) => Some(event),
            Waited::Idle => None,
            Waited::Stopped => return Ok(()),
        };
        let mut delivered = false;
        for event in first.into_iter().chain(events.try_iter()) {
            match event {
                Event::Send(message) => {
                    send_outbound(&mut carrier, session, message, elapsed(started))?
                }
                Event::Received(message, attachments) => {
                    delivered |= deliver(message, attachments, &inbound, session, elapsed(started));
                }
                Event::Shutdown => session.shutdown(elapsed(started)),
                Event::Ended(error) => {
                    disconnection = Some(error);
                    session.disconnected();
                }
            }
        }
        if delivered {
            inbound.repaint.request_repaint();
        }
        drain_outbound(&mut carrier, session)?;
        session.tick(elapsed(started));
        match session.state() {
            SessionState::Closed => return wait_for_shutdown(child),
            SessionState::Failed(failure) => {
                return Err(failed(failure.clone(), child, disconnection.take()))
            }
            _ => {}
        }
    }
}

fn send_outbound(
    carrier: &mut impl Writing,
    session: &mut HostSession,
    message: Message,
    now: u64,
) -> io::Result<()> {
    session
        .send(message, now)
        .map_err(|error| queue_error(&error))?;
    drain_outbound(carrier, session)
}

fn drain_outbound(carrier: &mut impl Writing, session: &mut HostSession) -> io::Result<()> {
    while let Some(message) = session.next_outbound() {
        eprintln!("plugin host sending {:?} to the plugin", &message);
        carrier.write(&message).map_err(carrier_error)?;
    }
    Ok(())
}

fn wait(events: &Receiver<Event>, session: &HostSession) -> Waited {
    let watching = session.pending_request_count() > 0
        || !matches!(session.state(), SessionState::Running | SessionState::Idle);
    if !watching {
        return match events.recv() {
            Ok(event) => Waited::Event(event),
            Err(_) => Waited::Stopped,
        };
    }
    match events.recv_timeout(TICK) {
        Ok(event) => Waited::Event(event),
        Err(RecvTimeoutError::Timeout) => Waited::Idle,
        Err(RecvTimeoutError::Disconnected) => Waited::Stopped,
    }
}

fn deliver(
    message: Message,
    attachments: Vec<Attachment>,
    inbound: &Inbound,
    session: &mut HostSession,
    now: u64,
) -> bool {
    if message.is_session() {
        session.receive(message, now);
        return false;
    }
    let received = match message {
        Message::Surface(surface) => {
            eprintln!(
                "plugin host received surface generation {} size={}x{} attachments={}",
                surface.generation,
                surface.width,
                surface.height,
                attachments.len()
            );
            Received::Surface(SurfaceEvent::Surface(surface, attachments))
        }
        Message::FrameReady(frame) => Received::Surface(SurfaceEvent::Frame(frame)),
        message => Received::Message(message),
    };
    inbound.received.send(received).is_ok()
}

fn failed(failure: SessionFailure, child: &mut Child, disconnection: Option<String>) -> io::Error {
    let exit = child.try_wait().ok().flatten();
    let reason = match (exit, disconnection) {
        (Some(exit), _) => format!("plugin exited unexpectedly: {exit}"),
        (None, Some(error)) => error,
        (None, None) => format!("plugin session failed: {failure:?}"),
    };
    io::Error::new(io::ErrorKind::ConnectionAborted, reason)
}

fn carrier_error(error: CarrierError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn queue_error(error: &block_plugin_api::QueueError) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("the plugin message queue failed: {error:?}"),
    )
}

fn encode(message: &Message) -> io::Result<Vec<u8>> {
    encode_frame(message).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "host message could not be encoded",
        )
    })
}

fn wait_for_shutdown(child: &mut Child) -> io::Result<()> {
    let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
    while Instant::now() < deadline {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "plugin did not exit after shutdown",
    ))
}

fn read_message(stream: &mut impl Read) -> io::Result<Message> {
    let mut header = [0; 4];
    stream.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    let mut frame = Vec::with_capacity(length + 4);
    frame.extend_from_slice(&header);
    frame.resize(length + 4, 0);
    stream.read_exact(&mut frame[4..])?;
    block_plugin_api::decode_frame(&frame)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "plugin sent a malformed frame"))
}

fn elapsed(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn terminate(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        child.kill().ok();
    }
    child.wait().ok();
}

#[cfg(test)]
mod tests;
