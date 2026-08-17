use std::sync::{Arc, Mutex};

use eframe::egui_wgpu::{self, wgpu};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PresenterState {
    Waiting,
    Presenting,
    Unsupported(String),
    Failed(String),
    Released,
}

#[derive(Clone)]
pub(super) struct PresenterStatus(Arc<Mutex<PresenterState>>);

impl PresenterStatus {
    pub(super) fn waiting() -> Self {
        Self(Arc::new(Mutex::new(PresenterState::Waiting)))
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn get(&self) -> PresenterState {
        self.0.lock().unwrap().clone()
    }

    fn set(&self, state: PresenterState) {
        *self.0.lock().unwrap() = state;
    }
}

pub(super) trait SurfacePresenter {
    type Frame;

    fn replace(&mut self, device: &wgpu::Device, frame: &Self::Frame) -> Result<(), String>;

    fn prepare(&mut self, queue: &wgpu::Queue, frame: &Self::Frame) -> Result<(), String>;

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>);

    fn release(&mut self);
}

pub(super) enum PresenterCommand<Frame> {
    Present(Frame),
    Release,
}

pub(super) struct PresenterCallback<Frame> {
    pub(super) command: PresenterCommand<Frame>,
    pub(super) status: PresenterStatus,
}

impl<Frame> egui_wgpu::CallbackTrait for PresenterCallback<Frame>
where
    Frame: Send + Sync + 'static,
    WebSurfacePresenter: SurfacePresenter<Frame = Frame>,
{
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(presenter) = resources.get_mut::<WebSurfacePresenter>() else {
            self.status.set(PresenterState::Unsupported(
                "The active renderer has no plugin surface presenter.".to_owned(),
            ));
            return Vec::new();
        };
        match &self.command {
            PresenterCommand::Present(frame) => {
                if let Err(error) = presenter.replace(device, frame) {
                    self.status.set(PresenterState::Failed(error));
                } else if let Err(error) = presenter.prepare(queue, frame) {
                    self.status.set(PresenterState::Failed(error));
                } else {
                    self.status.set(PresenterState::Presenting);
                }
            }
            PresenterCommand::Release => {
                presenter.release();
                self.status.set(PresenterState::Released);
            }
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if matches!(self.command, PresenterCommand::Present(_)) {
            if let Some(presenter) = resources.get::<WebSurfacePresenter>() {
                presenter.paint(render_pass);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) use super::web::renderer::{WebFrame, WebSurfacePresenter};
#[cfg(target_os = "windows")]
pub(super) use super::windows::WindowsSurfacePresenter as WebSurfacePresenter;
