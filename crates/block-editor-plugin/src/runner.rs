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
    #[cfg(target_os = "windows")]
    let result = run_endpoint::<A>(&arguments[2], id, name, version);
    #[cfg(not(target_os = "windows"))]
    let result = run_endpoint(&arguments[2], id, name, version);
    if let Err(error) = result {
        eprintln!("plugin transport failed: {error}");
        std::process::exit(1);
    }
}

fn transport(
    mut input: impl std::io::Read,
    mut output: impl std::io::Write,
    id: &str,
    name: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::native::{ClientSession, State};
    use block_plugin_api::{decode_frame, encode_frame, MAX_FRAME_BYTES};
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
        if length > MAX_FRAME_BYTES {
            return Err(format!("frame length {length} exceeds limit").into());
        }
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

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
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
    use crate::{native::ClientSession, windows_surface::Surface};
    use block_plugin_api::{
        decode_frame, desktop_attachments::WindowsAttachmentCarrier, encode_frame, Message,
        MAX_FRAME_BYTES,
    };
    use std::{
        io::{Read, Write},
        os::windows::io::AsRawHandle,
        time::Instant,
    };
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            Pipes::GetNamedPipeServerProcessId,
            Threading::{OpenProcess, PROCESS_DUP_HANDLE},
        },
    };

    let mut stream = connect(endpoint)?;
    eprintln!("connected to the Windows host");
    let mut session = ClientSession::new(id, name, version);
    stream.write_all(&encode_frame(&session.hello())?)?;
    stream.flush()?;
    let mut header = [0; 4];
    stream.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    if length > MAX_FRAME_BYTES {
        return Err("host handshake frame exceeds limit".into());
    }
    let mut frame = Vec::with_capacity(length + 4);
    frame.extend_from_slice(&header);
    frame.resize(length + 4, 0);
    stream.read_exact(&mut frame[4..])?;
    for response in session.receive(decode_frame(&frame)?) {
        stream.write_all(&encode_frame(&response)?)?;
    }
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
    let mut carrier = WindowsAttachmentCarrier::new(stream, host);
    let started = Instant::now();
    let mut surface: Option<Surface> = None;
    let mut generation = 0;
    let mut rendered_frames = 0_u64;
    loop {
        let (message, attachments) = carrier.receive()?;
        let incoming_name = message_name(&message);
        eprintln!("received {incoming_name} from Windows host");
        drop(attachments);
        let create = match &message {
            Message::CreateViewport(viewport) => {
                Some((viewport.request_id, viewport.metrics.clone()))
            }
            Message::ResizeViewport(metrics) => Some((1, metrics.clone())),
            _ => None,
        };
        if let (Message::Input(input), Some(surface)) = (&message, &mut surface) {
            surface.input(input);
        }
        if matches!(message, Message::Editor(_) | Message::Client(_)) {
            if let Some(surface) = &mut surface {
                for response in surface.receive(&message) {
                    carrier.send(&response, &[])?;
                }
            }
        }
        for response in session.receive(message) {
            carrier.send(&response, &[])?;
        }
        if let Some((request_id, metrics)) =
            create.filter(|(_, metrics)| metrics.pixel_width > 0 && metrics.pixel_height > 0)
        {
            eprintln!(
                "creating DXGI surface {}x{} at scale {}",
                metrics.pixel_width, metrics.pixel_height, metrics.scale_factor
            );
            generation += 1;
            let mut next = match surface.take() {
                Some(previous) => previous.resize::<A>(request_id, metrics, generation)?,
                None => Surface::new::<A>(request_id, metrics, generation)?,
            };
            eprintln!("created DXGI surface generation {generation}");
            let (descriptor, handles) = next.descriptor();
            carrier.send(&descriptor, &handles)?;
            eprintln!("transferred DXGI surface generation {generation}");
            for message in next.render(started.elapsed().as_secs_f64())? {
                carrier.send(&message, &[])?;
            }
            rendered_frames += 1;
            eprintln!(
                "rendered and sent DXGI frame {rendered_frames} for surface generation {generation}"
            );
            surface = Some(next);
        } else if let Some(surface) = &mut surface {
            for message in surface.render(started.elapsed().as_secs_f64())? {
                carrier.send(&message, &[])?;
            }
            rendered_frames += 1;
            eprintln!(
                "rendered and sent DXGI frame {rendered_frames} after {}",
                incoming_name
            );
        }
        if matches!(session.state(), crate::native::State::Closed) {
            unsafe { CloseHandle(host) };
            return Ok(());
        }
    }
}

#[cfg(target_os = "windows")]
fn message_name(message: &block_plugin_api::Message) -> &'static str {
    use block_plugin_api::Message;
    match message {
        Message::CreateViewport(_) => "CreateViewport",
        Message::ResizeViewport(_) => "ResizeViewport",
        Message::Input(_) => "Input",
        Message::Editor(_) => "Editor",
        Message::Client(_) => "Client",
        Message::Shutdown => "Shutdown",
        _ => "other message",
    }
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
