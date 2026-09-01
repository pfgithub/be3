use block_plugin_api::{EditorBand, Message};

use crate::{
    screens::Screens,
    session::{ClientSession, State},
    wasm::Surface,
    Waker,
};

pub(crate) struct Step {
    pub(crate) outbound: Vec<Message>,
    pub(crate) closed: bool,
}

pub(crate) struct Runtime {
    session: ClientSession,
    screens: Screens,
    surface: Option<Surface>,
    generation: u64,
    asked: bool,
}

impl Runtime {
    pub(crate) fn new<A: crate::App>(
        id: &str,
        name: &str,
        version: &str,
        chrome: Vec<EditorBand>,
        waker: Waker,
    ) -> Self {
        Self {
            session: ClientSession::new(id, name, version),
            screens: Screens::new::<A>(chrome, waker),
            surface: None,
            generation: 0,
            asked: false,
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
            if matches!(message, Message::DrawFrame) {
                draw = true;
            }
            changed |= self.screens.receive(&message);
            outbound.extend(self.session.receive(message));
            if matches!(self.session.state(), State::Closed) {
                return Ok(Step {
                    outbound,
                    closed: true,
                });
            }
        }
        block_client::pump();
        let replaced = self.replace_surface(&mut outbound)?;
        if let Some(surface) = &mut self.surface {
            if draw || replaced {
                self.asked = false;
                outbound.extend(surface.render(&mut self.screens, phase)?);
            } else if changed && !self.asked {
                self.asked = true;
                outbound.push(Message::FrameNeeded);
            }
        }
        outbound.extend(self.screens.outbound());
        Ok(Step {
            outbound,
            closed: false,
        })
    }

    fn replace_surface(&mut self, outbound: &mut Vec<Message>) -> Result<bool, String> {
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
            "creating a surface {}x{} for {} screens",
            layout.width,
            layout.height,
            layout.screens.len()
        ));
        self.surface = Some(match self.surface.take() {
            Some(previous) => previous.resize(layout.clone(), self.generation)?,
            None => Surface::new(layout.clone(), self.generation)?,
        });
        outbound.push(Message::Layout(layout));
        Ok(true)
    }
}

fn log(message: &str) {
    eprintln!("{message}");
}
