use block_plugin_api::{Message, PreviewLayout};

use crate::{
    platform::{Attachment, Surface, SHARED_PREVIEWS, SURFACE_KIND},
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
    pub(crate) closed: bool,
}

pub(crate) struct Runtime {
    session: ClientSession,
    screens: Screens,
    surface: Option<Surface>,
    generation: u64,
    request_id: u64,
    asked: bool,
    previews: PreviewLayout,
    preview_generation: u64,
}

impl Runtime {
    pub(crate) fn new<A: crate::App>(id: &str, name: &str, version: &str, waker: Waker) -> Self {
        Self {
            session: ClientSession::new(id, name, version),
            screens: Screens::new::<A>(waker),
            surface: None,
            generation: 0,
            request_id: 0,
            asked: false,
            previews: PreviewLayout::default(),
            preview_generation: 0,
        }
    }

    pub(crate) fn hello(&self) -> Message {
        self.session.hello()
    }

    pub(crate) fn step(
        &mut self,
        batch: Vec<Message>,
        woken: bool,
        phase: f64,
    ) -> Result<Step, String> {
        let mut changed = woken;
        let mut draw = false;
        let mut outbound = Vec::new();
        for message in batch {
            match &message {
                Message::Screens(set) => self.request_id = set.request_id,
                Message::DrawFrame => draw = true,
                Message::PreviewsReady { generation }
                    if *generation == self.previews.generation =>
                {
                    self.screens.set_previews(&self.previews);
                    changed = true;
                }
                _ => {}
            }
            changed |= self.screens.receive(&message);
            for response in self.session.receive(message) {
                outbound.push(plain(response));
            }
            if matches!(self.session.state(), State::Closed) {
                return Ok(Step {
                    outbound,
                    closed: true,
                });
            }
        }
        crate::platform::pump();
        let replaced = self.replace_surface(&mut outbound)?;
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
            if draw || replaced {
                self.asked = false;
                let messages = surface.render(&mut self.screens, phase)?;
                outbound.extend(messages.into_iter().map(plain));
            } else if changed && !self.asked {
                self.asked = true;
                outbound.push(plain(Message::FrameNeeded));
            }
        }
        self.update_previews(&mut outbound)?;
        outbound.extend(self.screens.outbound().into_iter().map(plain));
        Ok(Step {
            outbound,
            closed: false,
        })
    }

    fn update_previews(&mut self, outbound: &mut Vec<Outbound>) -> Result<(), String> {
        if !SHARED_PREVIEWS {
            return Ok(());
        }
        let requests = self.screens.preview_requests();
        let scale_factor = self.screens.scale_factor();
        let Some(surface) = &mut self.surface else {
            return Ok(());
        };
        let mut layout = PreviewLayout::packed(&requests, scale_factor);
        if layout.same_slots(&self.previews) {
            return Ok(());
        }
        self.preview_generation += 1;
        layout.generation = self.preview_generation;
        if let Some((message, attachments)) = surface.set_previews(&layout)? {
            log(&format!(
                "transferred a {SURFACE_KIND} preview surface {}x{}",
                layout.width, layout.height
            ));
            outbound.push(Outbound {
                message,
                attachments,
            });
        }
        self.previews = layout.clone();
        outbound.push(plain(Message::Previews(layout)));
        Ok(())
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
