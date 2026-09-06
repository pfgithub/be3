use std::error::Error;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::color::Color32;
use crate::context::Context;
use crate::geometry::{pos2, vec2, Pos2, Rect, Vec2};
use crate::input::{CursorIcon, Event, Key, Modifiers, PointerButton, RawInput};
use crate::renderer::{clear_color, Renderer};

const LINE_HEIGHT: f32 = 40.0;
const DEFAULT_SIZE: Vec2 = Vec2::new(1280.0, 800.0);

pub trait App {
    fn update(&mut self, context: &Context, rect: Rect);

    fn clear_color(&self) -> Color32 {
        Color32::BLACK
    }
}

pub fn run(title: impl Into<String>, app: impl App + 'static) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut runner = Runner {
        title: title.into(),
        app: Box::new(app),
        context: Context::new(),
        surface: None,
        events: Vec::new(),
        modifiers: Modifiers::NONE,
        pointer: Pos2::ZERO,
        error: None,
    };
    event_loop.run_app(&mut runner)?;
    match runner.error {
        Some(error) => Err(error.into()),
        None => Ok(()),
    }
}

struct Surface {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    cursor_icon: CursorIcon,
}

struct Runner {
    title: String,
    app: Box<dyn App>,
    context: Context,
    surface: Option<Surface>,
    events: Vec<Event>,
    modifiers: Modifiers,
    pointer: Pos2,
    error: Option<String>,
}

impl Runner {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl ToString) {
        self.error = Some(error.to_string());
        event_loop.exit();
    }

    fn push(&mut self, event: Event) {
        self.events.push(event);
        if let Some(surface) = &self.surface {
            surface.window.request_redraw();
        }
    }

    fn logical(&self, position: PhysicalPosition<f64>) -> Pos2 {
        let scale = self
            .surface
            .as_ref()
            .map_or(1.0, |surface| surface.window.scale_factor());
        pos2((position.x / scale) as f32, (position.y / scale) as f32)
    }

    fn redraw(&mut self) {
        let Some(surface) = &mut self.surface else {
            return;
        };
        if surface.config.width == 0 || surface.config.height == 0 {
            return;
        }

        let scale = surface.window.scale_factor() as f32;
        let physical = vec2(surface.config.width as f32, surface.config.height as f32);
        let screen = vec2(physical.x / scale, physical.y / scale);
        self.context.set_pixels_per_point(scale);

        let raw = RawInput {
            events: std::mem::take(&mut self.events),
        };
        let app = &mut self.app;
        let output = self.context.run(raw, |context| {
            app.update(context, Rect::from_min_size(Pos2::ZERO, screen));
        });

        if output.cursor_icon != surface.cursor_icon {
            surface.cursor_icon = output.cursor_icon;
            surface.window.set_cursor(cursor(output.cursor_icon));
        }

        surface
            .renderer
            .prepare(&surface.device, &surface.queue, &output, physical, scale);

        let frame = match surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                surface.surface.configure(&surface.device, &surface.config);
                return;
            }
            _ => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = surface
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beui encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("beui pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color(self.app.clear_color())),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            surface.renderer.paint(&mut pass);
        }
        surface.queue.submit(Some(encoder.finish()));
        frame.present();

        if output.repaint {
            surface.window.request_redraw();
        }
    }
}

impl ApplicationHandler for Runner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(LogicalSize::new(DEFAULT_SIZE.x, DEFAULT_SIZE.y));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => return self.fail(event_loop, error),
        };
        match pollster::block_on(create_surface(window)) {
            Ok(surface) => self.surface = Some(surface),
            Err(error) => self.fail(event_loop, error),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = self.surface.as_ref().map(|surface| surface.window.id());
        if window != Some(window_id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    surface.config.width = size.width;
                    surface.config.height = size.height;
                    if size.width > 0 && size.height > 0 {
                        surface.surface.configure(&surface.device, &surface.config);
                    }
                    surface.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(surface) = &self.surface {
                    surface.window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.modifiers = Modifiers {
                    alt: state.alt_key(),
                    ctrl: state.control_key(),
                    shift: state.shift_key(),
                };
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = self.logical(position);
                self.push(Event::PointerMoved(self.pointer));
            }
            WindowEvent::CursorLeft { .. } => self.push(Event::PointerGone),
            WindowEvent::MouseInput { state, button, .. } => {
                let Some(button) = pointer_button(button) else {
                    return;
                };
                self.push(Event::PointerButton {
                    pos: self.pointer,
                    button,
                    pressed: state == ElementState::Pressed,
                    modifiers: self.modifiers,
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => vec2(x * LINE_HEIGHT, y * LINE_HEIGHT),
                    MouseScrollDelta::PixelDelta(position) => {
                        vec2(position.x as f32, position.y as f32)
                    }
                };
                self.push(Event::Scroll(delta));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    if let Some(key) = key(code) {
                        self.push(Event::Key {
                            key,
                            pressed,
                            repeat: event.repeat,
                            modifiers: self.modifiers,
                        });
                    }
                }
                if pressed && !self.modifiers.ctrl && !self.modifiers.alt {
                    if let Some(text) = event.text {
                        if !text.chars().any(char::is_control) {
                            self.push(Event::Text(text.to_string()));
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }
}

async fn create_surface(window: Arc<Window>) -> Result<Surface, Box<dyn Error>> {
    let size = window.inner_size();
    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window.clone())?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("beui device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await?;
    let mut config = surface
        .get_default_config(&adapter, size.width.max(1), size.height.max(1))
        .ok_or("the adapter does not support this surface")?;
    let capabilities = surface.get_capabilities(&adapter);
    if let Some(format) = capabilities.formats.iter().copied().find(|it| it.is_srgb()) {
        config.format = format;
    }
    surface.configure(&device, &config);
    let renderer = Renderer::new(&device, config.format);

    Ok(Surface {
        window,
        surface,
        device,
        queue,
        config,
        renderer,
        cursor_icon: CursorIcon::Default,
    })
}

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        _ => None,
    }
}

fn cursor(icon: CursorIcon) -> winit::window::CursorIcon {
    match icon {
        CursorIcon::Default => winit::window::CursorIcon::Default,
        CursorIcon::Crosshair => winit::window::CursorIcon::Crosshair,
        CursorIcon::Grab => winit::window::CursorIcon::Grab,
        CursorIcon::Grabbing => winit::window::CursorIcon::Grabbing,
        CursorIcon::NotAllowed => winit::window::CursorIcon::NotAllowed,
        CursorIcon::PointingHand => winit::window::CursorIcon::Pointer,
        CursorIcon::ResizeHorizontal => winit::window::CursorIcon::EwResize,
        CursorIcon::ResizeVertical => winit::window::CursorIcon::NsResize,
        CursorIcon::Text => winit::window::CursorIcon::Text,
        CursorIcon::Wait => winit::window::CursorIcon::Wait,
    }
}

fn key(code: KeyCode) -> Option<Key> {
    let key = match code {
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::ArrowUp => Key::ArrowUp,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::End => Key::End,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Escape => Key::Escape,
        KeyCode::Home => Key::Home,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::Space => Key::Space,
        KeyCode::Tab => Key::Tab,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyB => Key::B,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D,
        KeyCode::KeyE => Key::E,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G,
        KeyCode::KeyH => Key::H,
        KeyCode::KeyI => Key::I,
        KeyCode::KeyJ => Key::J,
        KeyCode::KeyK => Key::K,
        KeyCode::KeyL => Key::L,
        KeyCode::KeyM => Key::M,
        KeyCode::KeyN => Key::N,
        KeyCode::KeyO => Key::O,
        KeyCode::KeyP => Key::P,
        KeyCode::KeyQ => Key::Q,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyT => Key::T,
        KeyCode::KeyU => Key::U,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyW => Key::W,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,
        _ => return None,
    };
    Some(key)
}
