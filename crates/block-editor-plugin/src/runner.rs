use std::{
    sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender},
    time::{Duration, Instant},
};

use block_plugin_api::{decode_frame, encode_frame, Message};

use crate::{
    platform::{self, Connection, Surface, SURFACE_KIND},
    screens::Screens,
    session::{ClientSession, State},
};

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
    let mut session = handshake(&mut connection, id, name, version)?;
    eprintln!("protocol handshake completed");
    let (reader, mut sender) = connection.split()?;
    let (events, incoming) = channel();
    spawn_reader(reader, events.clone())?;
    let started = Instant::now();
    let mut screens = Screens::new::<A>(waker(events));
    let mut surface: Option<Surface> = None;
    let mut generation = 0;
    let mut request_id = 0;
    let mut repaint_at: Option<Instant> = None;
    loop {
        let (batch, woken) = receive_batch(&incoming, repaint_at)?;
        let received = !batch.is_empty();
        for message in batch {
            if let Message::Screens(set) = &message {
                request_id = set.request_id;
            }
            screens.receive(&message);
            for response in session.receive(message) {
                sender.send(&response, &[])?;
            }
            if matches!(session.state(), State::Closed) {
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
                "creating a {SURFACE_KIND} surface {}x{} for {} screens",
                layout.width,
                layout.height,
                layout.screens.len()
            );
            surface = Some(match surface.take() {
                Some(previous) => previous.resize(request_id, layout.clone(), generation)?,
                None => Surface::new(request_id, layout.clone(), generation)?,
            });
            sender.send(&Message::Layout(layout), &[])?;
        }
        let due = repaint_at.is_some_and(|deadline| deadline <= Instant::now());
        if let Some(surface) = &mut surface {
            if replaced {
                let (descriptor, attachments) = surface.descriptor();
                sender.send(&descriptor, &attachments)?;
                eprintln!("transferred {SURFACE_KIND} surface generation {generation}");
            }
            if received || replaced || due || woken {
                let (messages, repaint) =
                    surface.render(&mut screens, started.elapsed().as_secs_f64())?;
                for message in messages {
                    sender.send(&message, &[])?;
                }
                repaint_at = repaint.map(|delay| Instant::now() + delay);
            }
        }
        for message in screens.outbound() {
            sender.send(&message, &[])?;
        }
    }
}

fn handshake(
    connection: &mut Connection,
    id: &str,
    name: &str,
    version: &str,
) -> Result<ClientSession, Box<dyn std::error::Error>> {
    use std::io::{Read, Write};
    let mut session = ClientSession::new(id, name, version);
    let writer = connection.writer();
    writer.write_all(&encode_frame(&session.hello())?)?;
    writer.flush()?;
    let reader = connection.reader();
    let mut header = [0; 4];
    reader.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    let mut frame = Vec::with_capacity(length + 4);
    frame.extend_from_slice(&header);
    frame.resize(length + 4, 0);
    reader.read_exact(&mut frame[4..])?;
    let responses = session.receive(decode_frame(&frame)?);
    let writer = connection.writer();
    for response in responses {
        writer.write_all(&encode_frame(&response)?)?;
    }
    writer.flush()?;
    Ok(session)
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
    deadline: Option<Instant>,
) -> Result<(Vec<Message>, bool), Box<dyn std::error::Error>> {
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

fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}
