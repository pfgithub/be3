use std::{
    io::{Read, Write},
    sync::mpsc::{channel, Receiver, Sender},
    time::Instant,
};

use block_plugin_api::{decode_frame, encode_frame, Message};

use crate::{platform, runtime::Runtime};

pub(crate) fn run<A: crate::App>(id: &str, name: &str, version: &str) {
    let arguments: Vec<_> = std::env::args().collect();
    if arguments.len() != 3 || arguments[1] != "--endpoint" {
        eprintln!("usage: {name} --endpoint ENDPOINT");
        std::process::exit(2);
    }
    if let Err(error) = run_endpoint::<A>(&arguments[2], id, name, version) {
        eprintln!("plugin transport failed: {error}");
        std::process::exit(1);
    }
}

enum Event {
    Received(Message),
    Woken,
    Failed(String),
}

fn run_endpoint<A: crate::App>(
    endpoint: &str,
    id: &str,
    name: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = platform::connect(endpoint)?;
    eprintln!("connected to the host");
    let (events, incoming) = channel();
    let mut runtime = Runtime::new::<A>(id, name, version, waker(events.clone()));
    let accepted = handshake(&mut connection, &runtime)?;
    eprintln!("protocol handshake completed");
    let (reader, mut sender) = connection.split()?;
    spawn_reader(reader, events)?;
    let started = Instant::now();
    let mut batch = vec![accepted];
    let mut woken = false;
    loop {
        let step = runtime.step(batch, woken, started.elapsed().as_secs_f64())?;
        for outbound in step.outbound {
            sender.send(&outbound.message, &outbound.attachments)?;
        }
        if step.closed {
            return Ok(());
        }
        let (received, wake) = receive_batch(&incoming)?;
        woken = wake;
        batch = received;
    }
}

fn handshake(
    connection: &mut platform::Connection,
    runtime: &Runtime,
) -> Result<Message, Box<dyn std::error::Error>> {
    let writer = connection.writer();
    writer.write_all(&encode_frame(&runtime.hello())?)?;
    writer.flush()?;
    let reader = connection.reader();
    let mut header = [0; 4];
    reader.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    let mut frame = Vec::with_capacity(length + 4);
    frame.extend_from_slice(&header);
    frame.resize(length + 4, 0);
    reader.read_exact(&mut frame[4..])?;
    Ok(decode_frame(&frame)?)
}

fn spawn_reader(
    mut reader: platform::Reader,
    events: Sender<Event>,
) -> Result<(), Box<dyn std::error::Error>> {
    std::thread::Builder::new()
        .name("plugin-reader".into())
        .spawn(move || loop {
            let event = match reader.receive() {
                Ok(message) => Event::Received(message),
                Err(error) => Event::Failed(error),
            };
            let failed = matches!(event, Event::Failed(_));
            if events.send(event).is_err() || failed {
                return;
            }
        })?;
    Ok(())
}

fn waker(events: Sender<Event>) -> crate::Waker {
    let events = std::sync::Mutex::new(events);
    crate::Waker::new(move || {
        if let Ok(events) = events.lock() {
            let _ = events.send(Event::Woken);
        }
    })
}

fn receive_batch(
    events: &Receiver<Event>,
) -> Result<(Vec<Message>, bool), Box<dyn std::error::Error>> {
    let first = events.recv()?;
    let mut messages = Vec::new();
    let mut woken = false;
    for event in [first].into_iter().chain(events.try_iter()) {
        match event {
            Event::Received(message) => messages.push(message),
            Event::Woken => woken = true,
            Event::Failed(error) => return Err(error.into()),
        }
    }
    Ok((messages, woken))
}
