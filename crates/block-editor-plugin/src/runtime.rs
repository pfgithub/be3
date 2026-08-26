use std::time::Duration;

use block_plugin_api::Message;

use crate::{
    platform::{Attachment, Surface, SURFACE_KIND},
    screens::Screens,
    session::{ClientSession, State},
    Waker,
};

pub(crate) struct Outbound {
    pub(crate) message: Message,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) attachments: Vec<Attachment>,
}

pub(crate) struct Step {
    pub(crate) outbound: Vec<Outbound>,
    pub(crate) repaint: Option<Duration>,
    pub(crate) closed: bool,
}

pub(crate) struct Runtime {
    session: ClientSession,
    screens: Screens,
    surface: Option<Surface>,
    generation: u64,
    request_id: u64,
}

impl Runtime {
    pub(crate) fn new<A: crate::App>(id: &str, name: &str, version: &str, waker: Waker) -> Self {
        Self {
            session: ClientSession::new(id, name, version),
            screens: Screens::new::<A>(waker),
            surface: None,
            generation: 0,
            request_id: 0,
        }
    }

    pub(crate) fn hello(&self) -> Message {
        self.session.hello()
    }

    pub(crate) fn step(
        &mut self,
        batch: Vec<Message>,
        draw: bool,
        phase: f64,
    ) -> Result<Step, String> {
        let mut changed = false;
        let mut outbound = Vec::new();
        for message in batch {
            if let Message::Screens(set) = &message {
                self.request_id = set.request_id;
            }
            changed |= self.screens.receive(&message);
            for response in self.session.receive(message) {
                outbound.push(plain(response));
            }
            if matches!(self.session.state(), State::Closed) {
                return Ok(Step {
                    outbound,
                    repaint: None,
                    closed: true,
                });
            }
        }
        let replaced = self.replace_surface(&mut outbound)?;
        let mut repaint = None;
        if let Some(surface) = &mut self.surface {
            if replaced {
                if let Some((descriptor, attachments)) = surface.descriptor() {
                    outbound.push(Outbound {
                        message: descriptor,
                        attachments,
                    });
                    log(&format!(
                        "transferred {SURFACE_KIND} surface generation {}",
                        self.generation
                    ));
                }
            }
            if changed || replaced || draw {
                let (messages, delay) = surface.render(&mut self.screens, phase)?;
                outbound.extend(messages.into_iter().map(plain));
                repaint = delay;
            }
        }
        outbound.extend(self.screens.outbound().into_iter().map(plain));
        Ok(Step {
            outbound,
            repaint,
            closed: false,
        })
    }

    fn replace_surface(&mut self, outbound: &mut Vec<Outbound>) -> Result<bool, String> {
        let layout = self.screens.layout().clone();
        if layout.is_empty()
            || self
                .surface
                .as_ref()
                .is_some_and(|surface| surface.layout().same_placements(&layout))
        {
            return Ok(false);
        }
        self.generation += 1;
        self.screens.set_generation(self.generation);
        let mut layout = layout;
        layout.generation = self.generation;
        log(&format!(
            "creating a {SURFACE_KIND} surface {}x{} for {} screens",
            layout.width,
            layout.height,
            layout.screens.len()
        ));
        self.surface = Some(match self.surface.take() {
            Some(previous) => previous.resize(self.request_id, layout.clone(), self.generation)?,
            None => Surface::new(self.request_id, layout.clone(), self.generation)?,
        });
        outbound.push(plain(Message::Layout(layout)));
        Ok(true)
    }
}

fn plain(message: Message) -> Outbound {
    Outbound {
        message,
        attachments: Vec::new(),
    }
}

fn log(message: &str) {
    eprintln!("{message}");
}
