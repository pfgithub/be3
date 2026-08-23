use block_plugin_api::{
    desktop_attachments::CarrierError, encode_frame, Capability, HostSession, Message, SessionState,
};
use std::{
    io::{self, BufRead, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
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

pub(super) enum Event {
    Send(Message),
    Shutdown,
    Acknowledged,
    Ended(String),
}

/// Reading a message the plugin sent, with whatever native resources came
/// with it. Implemented by each platform's carrier.
pub(super) trait Reading {
    fn read(&mut self) -> Result<(Message, Vec<Attachment>), CarrierError>;
}

/// Writing a message to the plugin. The host never sends native resources of
/// its own, so a carrier only has to carry the message.
pub(super) trait Writing {
    fn write(&mut self, message: &Message) -> Result<(), CarrierError>;
}

pub(super) struct Inbound {
    events: Sender<Event>,
    surfaces: Sender<SurfaceEvent>,
    layouts: Sender<block_plugin_api::ScreenLayout>,
    clients: Sender<block_plugin_api::TunnelMessage>,
    editors: Sender<block_plugin_api::EditorMessage>,
    sizes: Sender<Vec<block_plugin_api::RegionSize>>,
    repaint: eframe::egui::Context,
}

pub(super) struct Process {
    events: Sender<Event>,
    exit: Receiver<String>,
    surfaces: Receiver<SurfaceEvent>,
    layouts: Receiver<block_plugin_api::ScreenLayout>,
    clients: Receiver<block_plugin_api::TunnelMessage>,
    editors: Receiver<block_plugin_api::EditorMessage>,
    sizes: Receiver<Vec<block_plugin_api::RegionSize>>,
}

impl Process {
    pub(super) fn launch(executable: PathBuf, repaint: eframe::egui::Context) -> Self {
        let (events, event_receiver) = mpsc::channel();
        let (exit_sender, exit) = mpsc::channel();
        let (surface_sender, surfaces) = mpsc::channel();
        let (layout_sender, layouts) = mpsc::channel();
        let (client_sender, clients) = mpsc::channel();
        let (editor_sender, editors) = mpsc::channel();
        let (size_sender, sizes) = mpsc::channel();
        let event_sender = events.clone();
        thread::spawn(move || {
            let inbound = Inbound {
                events: event_sender,
                surfaces: surface_sender,
                layouts: layout_sender,
                clients: client_sender,
                editors: editor_sender,
                sizes: size_sender,
                repaint,
            };
            let reason = match run(executable, &event_receiver, inbound) {
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
            surfaces,
            layouts,
            clients,
            editors,
            sizes,
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

    pub(super) fn latest_surface(&self) -> Vec<SurfaceEvent> {
        let mut latest = Vec::new();
        for event in self.surfaces.try_iter() {
            match &event {
                SurfaceEvent::Surface(_, _) => {
                    latest.clear();
                    latest.push(event);
                }
                SurfaceEvent::Frame(_) => {
                    if matches!(latest.last(), Some(SurfaceEvent::Frame(_))) {
                        latest.pop();
                    }
                    latest.push(event);
                }
            }
        }
        latest
    }

    pub(super) fn layouts(&self) -> Vec<block_plugin_api::ScreenLayout> {
        self.layouts.try_iter().collect()
    }

    pub(super) fn client_messages(&self) -> Vec<block_plugin_api::TunnelMessage> {
        self.clients.try_iter().collect()
    }

    pub(super) fn editor_messages(&self) -> Vec<block_plugin_api::EditorMessage> {
        self.editors.try_iter().collect()
    }

    pub(super) fn region_sizes(&self) -> Vec<block_plugin_api::RegionSize> {
        self.sizes.try_iter().flatten().collect()
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Starts the plugin, speaks the handshake, and then reads and writes the
/// connection until either side ends it. The plugin is killed on the way out
/// however this returns.
fn run(executable: PathBuf, events: &Receiver<Event>, inbound: Inbound) -> io::Result<()> {
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
        .and_then(|connection| drive(connection, &mut child, events, inbound));
    terminate(&mut child);
    result
}

fn drive(
    mut connection: platform::Connection,
    child: &mut Child,
    events: &Receiver<Event>,
    inbound: Inbound,
) -> io::Result<()> {
    handshake(&mut connection)?;
    let (reader, writer) = connection.split(child)?;
    thread::spawn(move || read_from_plugin(reader, inbound));
    write_to_plugin(writer, child, events)
}

fn handshake(connection: &mut platform::Connection) -> io::Result<()> {
    let started = Instant::now();
    let mut session = HostSession::new(
        "BE3",
        vec![
            Capability::Lifecycle,
            Capability::Input,
            Capability::Surface(platform::SURFACE_MECHANISM),
        ],
    );
    session.start(0);
    let hello = read_message(connection.reader())?;
    session.receive(hello, elapsed(started));
    flush(connection.writer(), &mut session)?;
    match session.state() {
        SessionState::Running => Ok(()),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "plugin handshake was rejected",
        )),
    }
}

fn read_from_plugin(mut carrier: impl Reading, inbound: Inbound) {
    loop {
        let (message, attachments) = match carrier.read() {
            Ok(received) => received,
            Err(error) => {
                inbound.events.send(Event::Ended(error.to_string())).ok();
                return;
            }
        };
        match message {
            Message::Surface(surface) => {
                eprintln!(
                    "plugin host received surface generation {} size={}x{} attachments={}",
                    surface.generation,
                    surface.width,
                    surface.height,
                    attachments.len()
                );
                inbound
                    .surfaces
                    .send(SurfaceEvent::Surface(surface, attachments))
                    .ok();
            }
            Message::FrameReady(frame) => {
                inbound.surfaces.send(SurfaceEvent::Frame(frame)).ok();
            }
            Message::Layout(layout) => {
                eprintln!(
                    "plugin host received layout generation {} with {} screens",
                    layout.generation,
                    layout.screens.len()
                );
                inbound.layouts.send(layout).ok();
            }
            Message::ShutdownAcknowledged => {
                inbound.events.send(Event::Acknowledged).ok();
                return;
            }
            Message::Client(message) => {
                inbound.clients.send(message).ok();
            }
            Message::Editor(message) => {
                inbound.editors.send(message).ok();
            }
            Message::RegionSizes(message) => {
                inbound.sizes.send(message).ok();
            }
            _ => {}
        }
        inbound.repaint.request_repaint();
    }
}

fn write_to_plugin(
    mut carrier: impl Writing,
    child: &mut Child,
    events: &Receiver<Event>,
) -> io::Result<()> {
    loop {
        let Ok(first) = events.recv() else {
            return Ok(());
        };
        let mut outbound = Vec::new();
        let mut ending = None;
        for event in std::iter::once(first).chain(events.try_iter()) {
            match event {
                Event::Send(message) => outbound.push(message),
                event => {
                    ending = Some(event);
                    break;
                }
            }
        }
        coalesce_screens(&mut outbound);
        for message in outbound {
            if !matches!(
                message,
                Message::Input(_)
                    | Message::Editor(
                        block_plugin_api::EditorMessage::DragOver { .. }
                            | block_plugin_api::EditorMessage::ViewChanged { .. }
                            | block_plugin_api::EditorMessage::ChangeView { .. },
                    )
            ) {
                eprintln!("plugin host sending {} to the plugin", name(&message));
            }
            carrier.write(&message).map_err(carrier_error)?;
        }
        match ending {
            Some(Event::Shutdown) => {
                carrier.write(&Message::Shutdown).map_err(carrier_error)?;
                return wait_for_shutdown(child);
            }
            Some(Event::Acknowledged) => return Ok(()),
            Some(Event::Ended(error)) => {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    match child.try_wait()? {
                        Some(exit) => format!("plugin exited unexpectedly: {exit}"),
                        None => error,
                    },
                ))
            }
            _ => {}
        }
    }
}

fn coalesce_screens(messages: &mut Vec<Message>) {
    let Some(last) = messages
        .iter()
        .rposition(|message| matches!(message, Message::Screens(_)))
    else {
        return;
    };
    let mut index = 0;
    messages.retain(|message| {
        let keep = index == last || !matches!(message, Message::Screens(_));
        index += 1;
        keep
    });
}

fn name(message: &Message) -> &'static str {
    match message {
        Message::Screens(_) => "Screens",
        Message::Layout(_) => "Layout",
        Message::RegionSizes(_) => "RegionSizes",
        Message::Input(_) => "Input",
        Message::Editor(message) => editor_name(message),
        Message::Client(_) => "Client",
        Message::Shutdown => "Shutdown",
        _ => "a message",
    }
}

fn editor_name(message: &block_plugin_api::EditorMessage) -> &'static str {
    use block_plugin_api::EditorMessage;
    match message {
        EditorMessage::Open { .. } => "Editor::Open",
        EditorMessage::Close { .. } => "Editor::Close",
        EditorMessage::OpenCreation { .. } => "Editor::OpenCreation",
        EditorMessage::CommitCreation { .. } => "Editor::CommitCreation",
        EditorMessage::CreationBlock { .. } => "Editor::CreationBlock",
        EditorMessage::OpenArtifact { .. } => "Editor::OpenArtifact",
        EditorMessage::EditabilityChanged { .. } => "Editor::EditabilityChanged",
        EditorMessage::ViewChanged { .. } => "Editor::ViewChanged",
        EditorMessage::ChangeView { .. } => "Editor::ChangeView",
        _ => "Editor",
    }
}

fn carrier_error(error: CarrierError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
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

fn flush(stream: &mut impl Write, session: &mut HostSession) -> io::Result<()> {
    while let Some(message) = session.next_outbound() {
        let frame = encode_frame(&message).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "host message could not be encoded",
            )
        })?;
        stream.write_all(&frame)?;
    }
    stream.flush()
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
