use block_plugin_api::{
    encode_frame, Capability, HostSession, Message, SessionState, MAX_FRAME_BYTES,
};
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
use std::os::windows::io::OwnedHandle;

#[cfg(target_os = "windows")]
pub(super) enum SurfaceEvent {
    Surface(block_plugin_api::SurfaceDescriptor, Vec<OwnedHandle>),
    Frame(block_plugin_api::FrameReady),
}

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct Process {
    shutdown: Sender<()>,
    #[cfg(target_os = "windows")]
    messages: Sender<Message>,
    #[cfg(target_os = "windows")]
    surfaces: Receiver<SurfaceEvent>,
    #[cfg(target_os = "windows")]
    layouts: Receiver<block_plugin_api::ScreenLayout>,
    #[cfg(target_os = "windows")]
    clients: Receiver<block_plugin_api::TunnelMessage>,
}

impl Process {
    pub(super) fn launch(executable: PathBuf) -> Self {
        let (shutdown, shutdown_receiver) = mpsc::channel();
        #[cfg(target_os = "windows")]
        let (messages, message_receiver) = mpsc::channel();
        #[cfg(target_os = "windows")]
        let (surface_sender, surfaces) = mpsc::channel();
        #[cfg(target_os = "windows")]
        let (layout_sender, layouts) = mpsc::channel();
        #[cfg(target_os = "windows")]
        let (client_sender, clients) = mpsc::channel();
        thread::spawn(move || {
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
                    return drive_windows(
                        stream,
                        &mut child,
                        &shutdown_receiver,
                        &message_receiver,
                        &surface_sender,
                        &layout_sender,
                        &client_sender,
                    );
                    #[cfg(not(target_os = "windows"))]
                    drive(stream, &mut child, &shutdown_receiver)
                });
                terminate(&mut child);
                result
            });
            if let Err(error) = result {
                eprintln!("plugin host process failed: {error}");
            }
        });
        Self {
            shutdown,
            #[cfg(target_os = "windows")]
            messages,
            #[cfg(target_os = "windows")]
            surfaces,
            #[cfg(target_os = "windows")]
            layouts,
            #[cfg(target_os = "windows")]
            clients,
        }
    }

    pub(super) fn shutdown(&mut self) {
        self.shutdown.send(()).ok();
    }

    #[cfg(target_os = "windows")]
    pub(super) fn send(&self, messages: Vec<Message>) {
        for message in messages {
            self.messages.send(message).ok();
        }
    }

    #[cfg(target_os = "windows")]
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

    #[cfg(target_os = "windows")]
    pub(super) fn layouts(&self) -> Vec<block_plugin_api::ScreenLayout> {
        self.layouts.try_iter().collect()
    }

    #[cfg(target_os = "windows")]
    pub(super) fn client_messages(&self) -> Vec<block_plugin_api::TunnelMessage> {
        self.clients.try_iter().collect()
    }
}

#[cfg(target_os = "windows")]
fn drive_windows(
    mut stream: std::fs::File,
    child: &mut Child,
    shutdown: &Receiver<()>,
    messages: &Receiver<Message>,
    surfaces: &Sender<SurfaceEvent>,
    layouts: &Sender<block_plugin_api::ScreenLayout>,
    clients: &Sender<block_plugin_api::TunnelMessage>,
) -> io::Result<()> {
    use block_plugin_api::desktop_attachments::WindowsAttachmentCarrier;
    use std::os::windows::io::AsRawHandle;

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
    session.receive(read_message(&mut stream)?, elapsed(started));
    flush(&mut stream, &mut session)?;
    if !matches!(session.state(), SessionState::Running) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "plugin handshake was rejected",
        ));
    }
    let peer: windows_sys::Win32::Foundation::HANDLE = child.as_raw_handle().cast();
    let pipe: windows_sys::Win32::Foundation::HANDLE = stream.as_raw_handle().cast();
    let mut carrier = WindowsAttachmentCarrier::new(stream, peer);
    loop {
        if shutdown.try_recv().is_ok() {
            carrier
                .send(&Message::Shutdown, &[])
                .map_err(carrier_error)?;
            return wait_for_shutdown(child);
        }
        let mut outbound: Vec<Message> = messages.try_iter().collect();
        coalesce_screens(&mut outbound);
        for message in outbound {
            if !matches!(message, Message::Input(_)) {
                eprintln!("plugin host sending {} to the plugin", name(&message));
            }
            carrier.send(&message, &[]).map_err(carrier_error)?;
        }
        while pending(pipe)? {
            let (message, attachments) = carrier.receive().map_err(carrier_error)?;
            match message {
                Message::Surface(surface) => {
                    eprintln!(
                        "plugin host received DXGI surface generation {} size={}x{} attachments={}",
                        surface.generation,
                        surface.width,
                        surface.height,
                        attachments.len()
                    );
                    surfaces
                        .send(SurfaceEvent::Surface(surface, attachments))
                        .ok();
                }
                Message::FrameReady(frame) => {
                    surfaces.send(SurfaceEvent::Frame(frame)).ok();
                }
                Message::Layout(layout) => {
                    eprintln!(
                        "plugin host received layout generation {} with {} screens",
                        layout.generation,
                        layout.screens.len()
                    );
                    layouts.send(layout).ok();
                }
                Message::ShutdownAcknowledged => return Ok(()),
                Message::Client(message) => {
                    clients.send(message).ok();
                }
                _ => {}
            }
        }
        if let Some(exit) = child.try_wait()? {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                format!("plugin exited unexpectedly: {exit}"),
            ));
        }
        thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(target_os = "windows")]
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

#[cfg(target_os = "windows")]
fn pending(pipe: windows_sys::Win32::Foundation::HANDLE) -> io::Result<bool> {
    let mut available = 0;
    let peeked = unsafe {
        windows_sys::Win32::System::Pipes::PeekNamedPipe(
            pipe,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &raw mut available,
            std::ptr::null_mut(),
        )
    };
    if peeked == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(available > 0)
}

#[cfg(target_os = "windows")]
fn name(message: &Message) -> &'static str {
    match message {
        Message::Screens(_) => "Screens",
        Message::Layout(_) => "Layout",
        Message::Input(_) => "Input",
        Message::Editor(_) => "Editor",
        Message::Client(_) => "Client",
        Message::Shutdown => "Shutdown",
        _ => "a message",
    }
}

#[cfg(target_os = "windows")]
fn carrier_error(error: block_plugin_api::desktop_attachments::CarrierError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(target_os = "windows")]
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

#[cfg(not(target_os = "windows"))]
fn drive<S: Read + Write>(
    mut stream: S,
    child: &mut Child,
    shutdown: &Receiver<()>,
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
        if shutdown.try_recv().is_ok() {
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
        thread::sleep(Duration::from_millis(10));
    }
}

fn read_message(stream: &mut impl Read) -> io::Result<Message> {
    let mut header = [0; 4];
    stream.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "plugin sent an oversized frame",
        ));
    }
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
        Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX},
        System::Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_READMODE_BYTE,
            PIPE_TYPE_BYTE, PIPE_WAIT,
        },
    };
    pub(super) struct Endpoint {
        handle: HANDLE,
        name: String,
    }
    impl Endpoint {
        pub(super) fn create() -> io::Result<Self> {
            let name = format!(r"\\.\pipe\be3-plugin-{}-{}", std::process::id(), unique());
            let wide = wide(&name);
            let handle = unsafe {
                CreateNamedPipeW(
                    wide.as_ptr(),
                    PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
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
                Ok(Self { handle, name })
            }
        }
        pub(super) fn argument(&self) -> String {
            self.name.clone()
        }
        pub(super) fn accept(mut self, child: &Child) -> io::Result<File> {
            let connected = unsafe { ConnectNamedPipe(self.handle, std::ptr::null_mut()) };
            if connected == 0
                && io::Error::last_os_error().raw_os_error() != Some(ERROR_PIPE_CONNECTED as i32)
            {
                return Err(io::Error::last_os_error());
            }
            let mut process_id = 0;
            if unsafe { GetNamedPipeClientProcessId(self.handle, &raw mut process_id) } == 0 {
                return Err(io::Error::last_os_error());
            }
            if process_id != child.id() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "unexpected plugin peer",
                ));
            }
            let handle = std::mem::replace(&mut self.handle, INVALID_HANDLE_VALUE);
            Ok(unsafe { File::from_raw_handle(handle.cast()) })
        }
    }
    impl Drop for Endpoint {
        fn drop(&mut self) {
            if self.handle != INVALID_HANDLE_VALUE {
                unsafe { CloseHandle(self.handle) };
            }
        }
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
