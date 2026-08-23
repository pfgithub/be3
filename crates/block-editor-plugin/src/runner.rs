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
const FLUSH_WINDOW: std::time::Duration = std::time::Duration::from_millis(250);
#[cfg(any(target_os = "windows", target_os = "linux"))]
const FLUSH_POLL: std::time::Duration = std::time::Duration::from_millis(8);

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn wait_deadline(
    repaint_at: Option<std::time::Instant>,
    flush_until: Option<std::time::Instant>,
) -> Option<std::time::Instant> {
    let now = std::time::Instant::now();
    let flush_at = flush_until
        .filter(|until| *until > now)
        .map(|_| now + FLUSH_POLL);
    match (repaint_at, flush_at) {
        (Some(repaint), Some(flush)) => Some(repaint.min(flush)),
        (repaint, flush) => repaint.or(flush),
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
            loop {
                let event = match carrier.receive() {
                    Ok((message, _)) => Event::Received(message),
                    Err(error) => Event::Failed(error.to_string()),
                };
                let failed = matches!(event, Event::Failed(_));
                if events.send(event).is_err() || failed {
                    return;
                }
            }
        })?;
    Ok(())
}

#[cfg(target_os = "linux")]
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
    stream: &mut (impl std::io::Read + std::io::Write),
    id: &str,
    name: &str,
    version: &str,
) -> Result<crate::session::ClientSession, Box<dyn std::error::Error>> {
    use block_plugin_api::{decode_frame, encode_frame};
    let mut session = crate::session::ClientSession::new(id, name, version);
    stream.write_all(&encode_frame(&session.hello())?)?;
    stream.flush()?;
    let mut header = [0; 4];
    stream.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    let mut frame = Vec::with_capacity(length + 4);
    frame.extend_from_slice(&header);
    frame.resize(length + 4, 0);
    stream.read_exact(&mut frame[4..])?;
    for response in session.receive(decode_frame(&frame)?) {
        stream.write_all(&encode_frame(&response)?)?;
    }
    stream.flush()?;
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

    let mut stream = connect(endpoint)?;
    eprintln!("connected to the Linux host");
    let mut session = handshake(&mut stream, id, name, version)?;
    eprintln!("protocol handshake completed");
    let (sender, events) = std::sync::mpsc::channel();
    spawn_reader(stream.try_clone()?, sender.clone())?;
    let mut carrier = UnixAttachmentCarrier::new(stream);
    let started = Instant::now();
    let mut screens = crate::screens::Screens::new::<A>(waker(sender));
    let mut surface: Option<Surface> = None;
    let mut generation = 0;
    let mut request_id = 0;
    let mut repaint_at: Option<Instant> = None;
    let mut flush_until: Option<Instant> = None;
    loop {
        let (batch, woken) = receive_batch(&events, wait_deadline(repaint_at, flush_until))?;
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
        let outbound = screens.outbound();
        let flushed = !outbound.is_empty();
        for message in outbound {
            carrier.send(&message, &[])?;
        }
        if received || replaced || due || flushed || woken {
            flush_until = Some(Instant::now() + FLUSH_WINDOW);
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

    let mut stream = connect(endpoint)?;
    eprintln!("connected to the Windows host");
    let mut session = handshake(&mut stream, id, name, version)?;
    eprintln!("protocol handshake completed");
    let mut host_pid = 0;
    if unsafe { GetNamedPipeServerProcessId(stream.as_raw_handle().cast(), &mut host_pid) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let host = unsafe { OpenProcess(PROCESS_DUP_HANDLE, 0, host_pid) };
    if host.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    eprintln!("opened the host process for DXGI handle transfer");
    let pipe: windows_sys::Win32::Foundation::HANDLE = stream.as_raw_handle().cast();
    let mut carrier = WindowsAttachmentCarrier::new(stream, host);
    let started = Instant::now();
    let (sender, events) = std::sync::mpsc::channel();
    let mut screens = crate::screens::Screens::new::<A>(waker(sender));
    let mut surface: Option<Surface> = None;
    let mut generation = 0;
    let mut request_id = 0;
    let mut repaint_at: Option<Instant> = None;
    let mut flush_until: Option<Instant> = None;
    loop {
        let woken = wait_for_input(pipe, &events, wait_deadline(repaint_at, flush_until))?;
        let mut batch = Vec::new();
        while pending(pipe)? {
            batch.push(carrier.receive()?.0);
        }
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
        let outbound = screens.outbound();
        let flushed = !outbound.is_empty();
        for message in outbound {
            carrier.send(&message, &[])?;
        }
        if received || replaced || due || flushed || woken {
            flush_until = Some(Instant::now() + FLUSH_WINDOW);
        }
    }
}

#[cfg(target_os = "windows")]
fn wait_for_input(
    pipe: windows_sys::Win32::Foundation::HANDLE,
    events: &std::sync::mpsc::Receiver<Event>,
    deadline: Option<std::time::Instant>,
) -> std::io::Result<bool> {
    use std::sync::mpsc::RecvTimeoutError;
    const SHORTEST_POLL: std::time::Duration = std::time::Duration::from_millis(1);
    const LONGEST_POLL: std::time::Duration = std::time::Duration::from_millis(16);
    let mut woken = false;
    loop {
        while events.try_recv().is_ok() {
            woken = true;
        }
        if woken || pending(pipe)? {
            return Ok(woken);
        }
        let wait = match deadline {
            Some(deadline) if remaining(deadline).is_zero() => return Ok(false),
            Some(deadline) => remaining(deadline).clamp(SHORTEST_POLL, LONGEST_POLL),
            None => LONGEST_POLL,
        };
        match events.recv_timeout(wait) {
            Ok(_) => woken = true,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(woken),
        }
    }
}

#[cfg(target_os = "windows")]
fn pending(pipe: windows_sys::Win32::Foundation::HANDLE) -> std::io::Result<bool> {
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
        return Err(std::io::Error::last_os_error());
    }
    Ok(available > 0)
}

#[cfg(all(not(target_arch = "wasm32"), unix))]
fn connect(endpoint: &str) -> std::io::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(endpoint)
}

#[cfg(all(not(target_arch = "wasm32"), windows))]
fn connect(endpoint: &str) -> std::io::Result<std::fs::File> {
    use std::{
        ffi::OsStr,
        os::windows::{ffi::OsStrExt, io::FromRawHandle},
    };
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        Storage::FileSystem::{CreateFileW, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING},
    };
    let wide: Vec<u16> = OsStr::new(endpoint).encode_wide().chain(Some(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_GENERIC_READ | FILE_GENERIC_WRITE,
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
