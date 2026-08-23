pub(crate) fn run<A: crate::App>(id: &str, name: &str, version: &str) {
    let _ = std::marker::PhantomData::<A>;
    let arguments: Vec<_> = std::env::args().collect();
    if arguments.len() == 2 && arguments[1] == "--transport-test" {
        if let Err(error) = transport(
            std::io::stdin().lock(),
            std::io::stdout().lock(),
            id,
            name,
            version,
        ) {
            eprintln!("plugin transport failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if arguments.len() != 3 || arguments[1] != "--endpoint" {
        eprintln!("usage: counter --transport-test | --endpoint ENDPOINT");
        std::process::exit(2);
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let result = run_endpoint::<A>(&arguments[2], id, name, version);
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    let result = run_endpoint(&arguments[2], id, name, version);
    if let Err(error) = result {
        eprintln!("plugin transport failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn remaining(deadline: std::time::Instant) -> std::time::Duration {
    deadline.saturating_duration_since(std::time::Instant::now())
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
enum Event {
    Received(block_plugin_api::Message),
    Woken,
    Failed(String),
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn waker(events: std::sync::mpsc::Sender<Event>) -> crate::Waker {
    let events = std::sync::Mutex::new(events);
    crate::Waker::new(move || {
        if let Ok(events) = events.lock() {
            let _ = events.send(Event::Woken);
        }
    })
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn read_from_host(
    mut read: impl FnMut() -> Result<block_plugin_api::Message, String>,
    events: std::sync::mpsc::Sender<Event>,
) {
    loop {
        let event = match read() {
            Ok(message) => Event::Received(message),
            Err(error) => Event::Failed(error),
        };
        let failed = matches!(event, Event::Failed(_));
        if events.send(event).is_err() || failed {
            return;
        }
    }
}

#[cfg(target_os = "linux")]
fn spawn_reader(
    stream: std::os::unix::net::UnixStream,
    events: std::sync::mpsc::Sender<Event>,
) -> std::io::Result<()> {
    use block_plugin_api::desktop_attachments::UnixAttachmentCarrier;
    std::thread::Builder::new()
        .name("plugin-reader".into())
        .spawn(move || {
            let mut carrier = UnixAttachmentCarrier::new(stream);
            read_from_host(
                move || {
                    carrier
                        .receive()
                        .map(|(message, _)| message)
                        .map_err(|error| error.to_string())
                },
                events,
            );
        })?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn spawn_reader(
    stream: std::fs::File,
    events: std::sync::mpsc::Sender<Event>,
) -> std::io::Result<()> {
    use block_plugin_api::desktop_attachments::WindowsAttachmentCarrier;
    std::thread::Builder::new()
        .name("plugin-reader".into())
        .spawn(move || {
            let mut carrier = WindowsAttachmentCarrier::receiving(stream);
            read_from_host(
                move || {
                    carrier
                        .receive()
                        .map(|(message, _)| message)
                        .map_err(|error| error.to_string())
                },
                events,
            );
        })?;
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn receive_batch(
    events: &std::sync::mpsc::Receiver<Event>,
    deadline: Option<std::time::Instant>,
) -> Result<(Vec<block_plugin_api::Message>, bool), Box<dyn std::error::Error>> {
    use std::sync::mpsc::RecvTimeoutError;
    let first = match deadline {
        Some(deadline) => match events.recv_timeout(remaining(deadline)) {
            Ok(event) => Some(event),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return Err("the host reader stopped".into()),
        },
        None => Some(events.recv()?),
    };
    let mut messages = Vec::new();
    let mut woken = false;
    for event in first.into_iter().chain(events.try_iter()) {
        match event {
            Event::Received(message) => messages.push(message),
            Event::Woken => woken = true,
            Event::Failed(error) => return Err(error.into()),
        }
    }
    Ok((messages, woken))
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn handshake(
    reader: &mut impl std::io::Read,
    writer: &mut impl std::io::Write,
    id: &str,
    name: &str,
    version: &str,
) -> Result<crate::session::ClientSession, Box<dyn std::error::Error>> {
    use block_plugin_api::{decode_frame, encode_frame};
    let mut session = crate::session::ClientSession::new(id, name, version);
    writer.write_all(&encode_frame(&session.hello())?)?;
    writer.flush()?;
    let mut header = [0; 4];
    reader.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    let mut frame = Vec::with_capacity(length + 4);
    frame.extend_from_slice(&header);
    frame.resize(length + 4, 0);
    reader.read_exact(&mut frame[4..])?;
    for response in session.receive(decode_frame(&frame)?) {
        writer.write_all(&encode_frame(&response)?)?;
    }
    writer.flush()?;
    Ok(session)
}

fn transport(
    mut input: impl std::io::Read,
    mut output: impl std::io::Write,
    id: &str,
    name: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::session::{ClientSession, State};
    use block_plugin_api::{decode_frame, encode_frame};
    let mut session = ClientSession::new(id, name, version);
    output.write_all(&encode_frame(&session.hello())?)?;
    output.flush()?;

    loop {
        let mut header = [0; 4];
        match input.read_exact(&mut header) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error.into()),
        }
        let length = u32::from_be_bytes(header) as usize;
        let mut frame = Vec::with_capacity(length + 4);
        frame.extend_from_slice(&header);
        frame.resize(length + 4, 0);
        input.read_exact(&mut frame[4..])?;
        let message = decode_frame(&frame)?;
        for response in session.receive(message) {
            output.write_all(&encode_frame(&response)?)?;
        }
        output.flush()?;
        match session.state() {
            State::Closed => return Ok(()),
            State::Failed => return Err("protocol violation".into()),
            State::AwaitingHello | State::Running => {}
        }
    }
}

#[cfg(target_os = "linux")]
fn run_endpoint<A: crate::App>(
    endpoint: &str,
    id: &str,
    name: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::linux_surface::Surface;
    use block_plugin_api::{desktop_attachments::UnixAttachmentCarrier, Message};
    use std::time::Instant;

    let mut writer = connect(endpoint)?;
    let mut reader = writer.try_clone()?;
    eprintln!("connected to the Linux host");
    let mut session = handshake(&mut reader, &mut writer, id, name, version)?;
    eprintln!("protocol handshake completed");
    let (sender, events) = std::sync::mpsc::channel();
    spawn_reader(reader, sender.clone())?;
    let mut carrier = UnixAttachmentCarrier::new(writer);
    let started = Instant::now();
    let mut screens = crate::screens::Screens::new::<A>(waker(sender));
    let mut surface: Option<Surface> = None;
    let mut generation = 0;
    let mut request_id = 0;
    let mut repaint_at: Option<Instant> = None;
    loop {
        let (batch, woken) = receive_batch(&events, repaint_at)?;
        let received = !batch.is_empty();
        for message in batch {
            if let Message::Screens(set) = &message {
                request_id = set.request_id;
            }
            screens.receive(&message);
            for response in session.receive(message) {
                carrier.send(&response, &[])?;
            }
            if matches!(session.state(), crate::session::State::Closed) {
                return Ok(());
            }
        }
        let layout = screens.layout().clone();
        let replaced = !layout.is_empty()
            && !surface
                .as_ref()
                .is_some_and(|surface| surface.layout().same_placements(&layout));
        if replaced {
            generation += 1;
            screens.set_generation(generation);
            let mut layout = layout;
            layout.generation = generation;
            eprintln!(
                "creating a dma-buf surface {}x{} for {} screens",
                layout.width,
                layout.height,
                layout.screens.len()
            );
            surface = Some(match surface.take() {
                Some(previous) => previous.resize(request_id, layout.clone(), generation)?,
                None => Surface::new(request_id, layout.clone(), generation)?,
            });
            carrier.send(&Message::Layout(layout), &[])?;
        }
        let due = repaint_at.is_some_and(|deadline| deadline <= Instant::now());
        if let Some(surface) = &mut surface {
            if replaced {
                let (descriptor, planes) = surface.descriptor();
                carrier.send(&descriptor, &planes)?;
                eprintln!("transferred dma-buf surface generation {generation}");
            }
            if received || replaced || due || woken {
                let (messages, repaint) =
                    surface.render(&mut screens, started.elapsed().as_secs_f64())?;
                for message in messages {
                    carrier.send(&message, &[])?;
                }
                repaint_at = repaint.map(|delay| Instant::now() + delay);
            }
        }
        for message in screens.outbound() {
            carrier.send(&message, &[])?;
        }
    }
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "windows"),
    not(target_os = "linux")
))]
fn run_endpoint(
    endpoint: &str,
    id: &str,
    name: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream = connect(endpoint)?;
    transport(stream.try_clone()?, stream, id, name, version)
}

#[cfg(target_os = "windows")]
fn run_endpoint<A: crate::App>(
    endpoint: &str,
    id: &str,
    name: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::windows_surface::Surface;
    use block_plugin_api::{desktop_attachments::WindowsAttachmentCarrier, Message};
    use std::{os::windows::io::AsRawHandle, time::Instant};
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            Pipes::GetNamedPipeServerProcessId,
            Threading::{OpenProcess, PROCESS_DUP_HANDLE},
        },
    };

    let (mut reader, mut writer) = connect(endpoint)?;
    eprintln!("connected to the Windows host");
    let mut session = handshake(&mut reader, &mut writer, id, name, version)?;
    eprintln!("protocol handshake completed");
    let mut host_pid = 0;
    if unsafe { GetNamedPipeServerProcessId(writer.as_raw_handle().cast(), &mut host_pid) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let host = unsafe { OpenProcess(PROCESS_DUP_HANDLE, 0, host_pid) };
    if host.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    eprintln!("opened the host process for DXGI handle transfer");
    let (sender, events) = std::sync::mpsc::channel();
    spawn_reader(reader, sender.clone())?;
    let mut carrier = WindowsAttachmentCarrier::new(writer, host);
    let started = Instant::now();
    let mut screens = crate::screens::Screens::new::<A>(waker(sender));
    let mut surface: Option<Surface> = None;
    let mut generation = 0;
    let mut request_id = 0;
    let mut repaint_at: Option<Instant> = None;
    loop {
        let (batch, woken) = receive_batch(&events, repaint_at)?;
        let received = !batch.is_empty();
        for message in batch {
            if let Message::Screens(set) = &message {
                request_id = set.request_id;
            }
            screens.receive(&message);
            for response in session.receive(message) {
                carrier.send(&response, &[])?;
            }
            if matches!(session.state(), crate::session::State::Closed) {
                unsafe { CloseHandle(host) };
                return Ok(());
            }
        }
        let layout = screens.layout().clone();
        let replaced = !layout.is_empty()
            && !surface
                .as_ref()
                .is_some_and(|surface| surface.layout().same_placements(&layout));
        if replaced {
            generation += 1;
            screens.set_generation(generation);
            let mut layout = layout;
            layout.generation = generation;
            eprintln!(
                "creating DXGI surface {}x{} for {} screens",
                layout.width,
                layout.height,
                layout.screens.len()
            );
            surface = Some(match surface.take() {
                Some(previous) => previous.resize(request_id, layout.clone(), generation)?,
                None => Surface::new(request_id, layout.clone(), generation)?,
            });
            carrier.send(&Message::Layout(layout), &[])?;
        }
        let due = repaint_at.is_some_and(|deadline| deadline <= Instant::now());
        if let Some(surface) = &mut surface {
            if replaced {
                let (descriptor, handles) = surface.descriptor();
                carrier.send(&descriptor, &handles)?;
                eprintln!("transferred DXGI surface generation {generation}");
            }
            if received || replaced || due || woken {
                let (messages, repaint) =
                    surface.render(&mut screens, started.elapsed().as_secs_f64())?;
                for message in messages {
                    carrier.send(&message, &[])?;
                }
                repaint_at = repaint.map(|delay| Instant::now() + delay);
            }
        }
        for message in screens.outbound() {
            carrier.send(&message, &[])?;
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), unix))]
fn connect(endpoint: &str) -> std::io::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(endpoint)
}

#[cfg(all(not(target_arch = "wasm32"), windows))]
fn connect(endpoint: &str) -> std::io::Result<(std::fs::File, std::fs::File)> {
    use windows_sys::Win32::Storage::FileSystem::{FILE_GENERIC_READ, FILE_GENERIC_WRITE};
    let reader = open(&format!("{endpoint}-to-plugin"), FILE_GENERIC_READ)?;
    let writer = open(&format!("{endpoint}-to-host"), FILE_GENERIC_WRITE)?;
    Ok((reader, writer))
}

#[cfg(all(not(target_arch = "wasm32"), windows))]
fn open(name: &str, access: u32) -> std::io::Result<std::fs::File> {
    use std::{
        ffi::OsStr,
        os::windows::{ffi::OsStrExt, io::FromRawHandle},
    };
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        Storage::FileSystem::{CreateFileW, OPEN_EXISTING},
    };
    let wide: Vec<u16> = OsStr::new(name).encode_wide().chain(Some(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            access,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { std::fs::File::from_raw_handle(handle.cast()) })
    }
}
