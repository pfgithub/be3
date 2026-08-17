#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let arguments: Vec<_> = std::env::args().collect();
    if arguments.len() != 2 || arguments[1] != "--transport-test" {
        eprintln!("usage: plugin-demo --transport-test");
        std::process::exit(2);
    }
    if let Err(error) = run() {
        eprintln!("plugin transport failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use block_plugin_api::{decode_frame, encode_frame, MAX_FRAME_BYTES};
    use plugin_demo::native::{ClientSession, State};
    use std::io::{Read, Write};

    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut session = ClientSession::default();
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

#[cfg(target_arch = "wasm32")]
fn main() {}
