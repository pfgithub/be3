use block_plugin_api::{encode_frame, Capability, HostSession, Message, SessionState};
use std::{
    io,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use std::io::{BufRead, Read, Write};

#[cfg(target_os = "windows")]
pub(super) type Attachment = std::os::windows::io::OwnedHandle;
#[cfg(target_os = "linux")]
pub(super) type Attachment = std::os::fd::OwnedFd;

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

#[cfg(any(target_os = "windows", target_os = "linux"))]
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
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            let inbound = Inbound {
                events: event_sender,
                surfaces: surface_sender,
                layouts: layout_sender,
                clients: client_sender,
                editors: editor_sender,
                sizes: size_sender,
                repaint,
            };
            let result = platform::Endpoint::create().and_then(|endpoint| {
                let argument = endpoint.argument();
                let mut command = Command::new(&executable);
                command
                    .args(["--endpoint", &argument])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped());
                #[cfg(target_os = "windows")]
                if let Some(directory) = executable.parent() {
                    command.current_dir(directory).env_remove("PATH");
                }
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
                let result = endpoint.accept(&child).and_then(|stream| {
                    #[cfg(target_os = "windows")]
                    return drive_windows(stream, &mut child, &event_receiver, inbound);
                    #[cfg(target_os = "linux")]
                    return drive_unix(stream, &mut child, &event_receiver, inbound);
                    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
                    drive(stream, &mut child, &event_receiver)
                });
                terminate(&mut child);
                result
            });
            let reason = match result {
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

#[cfg(any(target_os = "windows", target_os = "linux"))]
trait Reading {
    fn read(
        &mut self,
    ) -> Result<(Message, Vec<Attachment>), block_plugin_api::desktop_attachments::CarrierError>;
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
trait Writing {
    fn write(
        &mut self,
        message: &Message,
    ) -> Result<(), block_plugin_api::desktop_attachments::CarrierError>;
}

#[cfg(target_os = "linux")]
impl Reading for block_plugin_api::desktop_attachments::UnixAttachmentCarrier {
    fn read(
        &mut self,
    ) -> Result<(Message, Vec<Attachment>), block_plugin_api::desktop_attachments::CarrierError>
    {
        self.receive()
    }
}

#[cfg(target_os = "linux")]
impl Writing for block_plugin_api::desktop_attachments::UnixAttachmentCarrier {
    fn write(
        &mut self,
        message: &Message,
    ) -> Result<(), block_plugin_api::desktop_attachments::CarrierError> {
        self.send(message, &[])
    }
}

#[cfg(target_os = "windows")]
impl Reading for block_plugin_api::desktop_attachments::WindowsAttachmentCarrier {
    fn read(
        &mut self,
    ) -> Result<(Message, Vec<Attachment>), block_plugin_api::desktop_attachments::CarrierError>
    {
        self.receive()
    }
}

#[cfg(target_os = "windows")]
impl Writing for block_plugin_api::desktop_attachments::WindowsAttachmentCarrier {
    fn write(
        &mut self,
        message: &Message,
    ) -> Result<(), block_plugin_api::desktop_attachments::CarrierError> {
        self.send(message, &[])
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
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

#[cfg(any(target_os = "windows", target_os = "linux"))]
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

#[cfg(target_os = "windows")]
fn drive_windows(
    streams: (std::fs::File, std::fs::File),
    child: &mut Child,
    events: &Receiver<Event>,
    inbound: Inbound,
) -> io::Result<()> {
    use block_plugin_api::desktop_attachments::WindowsAttachmentCarrier;
    use std::os::windows::io::AsRawHandle;

    let (mut reader, mut writer) = streams;
    let started = Instant::now();
    let mut session = HostSession::new(
        "BE3",
        vec![
            Capability::Lifecycle,
            Capability::Input,
            Capability::Surface(block_plugin_api::SurfaceMechanism::WindowsDxgi),
        ],
    );
    session.start(0);
    session.receive(read_message(&mut reader)?, elapsed(started));
    flush(&mut writer, &mut session)?;
    if !matches!(session.state(), SessionState::Running) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "plugin handshake was rejected",
        ));
    }
    let peer: windows_sys::Win32::Foundation::HANDLE = child.as_raw_handle().cast();
    thread::spawn(move || {
        read_from_plugin(WindowsAttachmentCarrier::receiving(reader), inbound);
    });
    write_to_plugin(WindowsAttachmentCarrier::new(writer, peer), child, events)
}

#[cfg(target_os = "linux")]
fn drive_unix(
    mut stream: std::os::unix::net::UnixStream,
    child: &mut Child,
    events: &Receiver<Event>,
    inbound: Inbound,
) -> io::Result<()> {
    use block_plugin_api::desktop_attachments::UnixAttachmentCarrier;

    let started = Instant::now();
    let mut session = HostSession::new(
        "BE3",
        vec![
            Capability::Lifecycle,
            Capability::Input,
            Capability::Surface(block_plugin_api::SurfaceMechanism::LinuxDmaBuf),
        ],
    );
    session.start(0);
    session.receive(read_message(&mut stream)?, elapsed(started));
    flush(&mut stream, &mut session)?;
    if !matches!(session.state(), SessionState::Running) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "plugin handshake was rejected",
        ));
    }
    let reader = stream.try_clone()?;
    thread::spawn(move || {
        read_from_plugin(UnixAttachmentCarrier::new(reader), inbound);
    });
    write_to_plugin(UnixAttachmentCarrier::new(stream), child, events)
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

fn carrier_error(error: block_plugin_api::desktop_attachments::CarrierError) -> io::Error {
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

impl Drop for Process {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn drive<S: Read + Write>(
    mut stream: S,
    child: &mut Child,
    events: &Receiver<Event>,
) -> io::Result<()> {
    let started = Instant::now();
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
    let mut session = HostSession::new("BE3", capabilities);
    session.start(0);
    let message = read_message(&mut stream)?;
    session.receive(message, elapsed(started));
    flush(&mut stream, &mut session)?;
    if !matches!(session.state(), SessionState::Running) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "plugin handshake was rejected",
        ));
    }
    loop {
        if matches!(
            events.recv_timeout(Duration::from_millis(10)),
            Ok(Event::Shutdown)
        ) {
            session.shutdown(elapsed(started));
            flush(&mut stream, &mut session)?;
            stream.flush()?;
            let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
            while Instant::now() < deadline {
                if child.try_wait()?.is_some() {
                    return Ok(());
                }
                thread::sleep(Duration::from_millis(10));
            }
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "plugin did not exit after shutdown",
            ));
        }
        if let Some(exit) = child.try_wait()? {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                format!("plugin exited unexpectedly: {exit}"),
            ));
        }
    }
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

#[cfg(unix)]
mod platform {
    use std::{
        fs, io,
        os::unix::net::{UnixListener, UnixStream},
        path::PathBuf,
        process::Child,
    };

    pub(super) struct Endpoint {
        listener: UnixListener,
        path: PathBuf,
    }

    impl Endpoint {
        pub(super) fn create() -> io::Result<Self> {
            let path = std::env::temp_dir().join(format!(
                "be3-plugin-{}-{}.sock",
                std::process::id(),
                unique()
            ));
            let listener = UnixListener::bind(&path)?;
            Ok(Self { listener, path })
        }

        pub(super) fn argument(&self) -> String {
            self.path.to_string_lossy().into_owned()
        }

        pub(super) fn accept(&self, child: &Child) -> io::Result<UnixStream> {
            let (stream, _) = self.listener.accept()?;
            verify_peer(&stream, child)?;
            Ok(stream)
        }
    }

    impl Drop for Endpoint {
        fn drop(&mut self) {
            fs::remove_file(&self.path).ok();
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn verify_peer(stream: &UnixStream, child: &Child) -> io::Result<()> {
        use std::os::fd::AsRawFd;
        let mut credentials = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&raw mut credentials).cast(),
                &raw mut length,
            )
        };
        if result == 0 && credentials.pid as u32 == child.id() {
            Ok(())
        } else if result != 0 {
            Err(io::Error::last_os_error())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unexpected plugin peer",
            ))
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    fn verify_peer(_stream: &UnixStream, _child: &Child) -> io::Result<()> {
        Ok(())
    }

    fn unique() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    }
}

#[cfg(windows)]
mod platform {
    use std::{
        ffi::OsStr,
        fs::File,
        io,
        os::windows::{ffi::OsStrExt, io::FromRawHandle},
        process::Child,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND, PIPE_ACCESS_OUTBOUND,
        },
        System::Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_READMODE_BYTE,
            PIPE_TYPE_BYTE, PIPE_WAIT,
        },
    };
    pub(super) struct Endpoint {
        inbound: HANDLE,
        outbound: HANDLE,
        name: String,
    }
    impl Endpoint {
        pub(super) fn create() -> io::Result<Self> {
            let name = format!(r"\\.\pipe\be3-plugin-{}-{}", std::process::id(), unique());
            let mut endpoint = Self {
                inbound: INVALID_HANDLE_VALUE,
                outbound: INVALID_HANDLE_VALUE,
                name,
            };
            endpoint.inbound = pipe(&super::to_host(&endpoint.name), PIPE_ACCESS_INBOUND)?;
            endpoint.outbound = pipe(&super::to_plugin(&endpoint.name), PIPE_ACCESS_OUTBOUND)?;
            Ok(endpoint)
        }
        pub(super) fn argument(&self) -> String {
            self.name.clone()
        }
        pub(super) fn accept(mut self, child: &Child) -> io::Result<(File, File)> {
            connect(self.inbound, child)?;
            connect(self.outbound, child)?;
            let inbound = std::mem::replace(&mut self.inbound, INVALID_HANDLE_VALUE);
            let outbound = std::mem::replace(&mut self.outbound, INVALID_HANDLE_VALUE);
            Ok(unsafe {
                (
                    File::from_raw_handle(inbound.cast()),
                    File::from_raw_handle(outbound.cast()),
                )
            })
        }
    }
    impl Drop for Endpoint {
        fn drop(&mut self) {
            for handle in [self.inbound, self.outbound] {
                if handle != INVALID_HANDLE_VALUE {
                    unsafe { CloseHandle(handle) };
                }
            }
        }
    }
    fn pipe(name: &str, access: u32) -> io::Result<HANDLE> {
        let wide = wide(name);
        let handle = unsafe {
            CreateNamedPipeW(
                wide.as_ptr(),
                access | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                1,
                1_048_580,
                1_048_580,
                5_000,
                std::ptr::null(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(handle)
        }
    }
    fn connect(handle: HANDLE, child: &Child) -> io::Result<()> {
        let connected = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) };
        if connected == 0
            && io::Error::last_os_error().raw_os_error() != Some(ERROR_PIPE_CONNECTED as i32)
        {
            return Err(io::Error::last_os_error());
        }
        let mut process_id = 0;
        if unsafe { GetNamedPipeClientProcessId(handle, &raw mut process_id) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if process_id != child.id() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unexpected plugin peer",
            ));
        }
        Ok(())
    }
    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain(Some(0)).collect()
    }
    fn unique() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    }
}

#[cfg(windows)]
fn to_host(endpoint: &str) -> String {
    format!("{endpoint}-to-host")
}

#[cfg(windows)]
fn to_plugin(endpoint: &str) -> String {
    format!("{endpoint}-to-plugin")
}
