use block_plugin_api::{
    encode_frame, Capability, HostSession, Message, SessionState, MAX_FRAME_BYTES,
};
use std::{
    io::{self, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct Process {
    status: Receiver<String>,
    shutdown: Sender<()>,
    latest: String,
}

impl Process {
    pub(super) fn launch(executable: PathBuf) -> Self {
        let (status_sender, status) = mpsc::channel();
        let (shutdown, shutdown_receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = platform::Endpoint::create().and_then(|endpoint| {
                let argument = endpoint.argument();
                let mut child = Command::new(&executable)
                    .args(["--endpoint", &argument])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|error| {
                        io::Error::new(
                            error.kind(),
                            format!("failed to launch {}: {error}", executable.display()),
                        )
                    })?;
                status_sender
                    .send("Waiting for plugin handshake".into())
                    .ok();
                let result = endpoint.accept(&child).and_then(|stream| {
                    drive(stream, &mut child, &shutdown_receiver, &status_sender)
                });
                terminate(&mut child);
                result
            });
            if let Err(error) = result {
                status_sender.send(error.to_string()).ok();
            }
        });
        Self {
            status,
            shutdown,
            latest: "Starting plugin process".into(),
        }
    }

    pub(super) fn status(&mut self) -> String {
        while let Ok(status) = self.status.try_recv() {
            self.latest = status;
        }
        self.latest.clone()
    }

    pub(super) fn shutdown(&mut self) {
        self.shutdown.send(()).ok();
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn drive<S: Read + Write>(
    mut stream: S,
    child: &mut Child,
    shutdown: &Receiver<()>,
    status: &Sender<String>,
) -> io::Result<()> {
    let started = Instant::now();
    let mut session = HostSession::new("BE3", vec![Capability::Lifecycle, Capability::Input]);
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
    status
        .send("Plugin connected; no compatible desktop surface presenter is available".into())
        .ok();
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
