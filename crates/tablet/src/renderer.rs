use crate::apps::calendar::CalendarApp;
use crate::apps::notes::NotesApp;
use crate::text::TextEngine;
use bytemuck::{Pod, Zeroable};
use std::ops::{Add, Index, Sub};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

const INITIAL_WIDTH: u32 = 900;
const INITIAL_HEIGHT: u32 = 520;
const ATLAS_SIZE: u32 = 2048;
const STATUS_BAR_HEIGHT: f32 = 54.0;
const HOME_BUTTON_WIDTH: f32 = 128.0;
const OUTER_MARGIN: f32 = 22.0;
const BUTTON_GAP: f32 = 18.0;

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let text_engine = TextEngine::new().ok_or("no supported system font was found")?;
    let event_loop = EventLoop::new()?;
    let mut app = Application {
        text_engine,
        ui: TabletUi::new(),
        window: None,
        renderer: None,
        error: None,
    };
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error.into());
    }
    Ok(())
}

struct Application {
    text_engine: TextEngine,
    ui: TabletUi,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    error: Option<String>,
}

impl Application {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl ToString) {
        self.error = Some(error.to_string());
        event_loop.exit();
    }

    fn draw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let mut scene = Scene::new(Vector::new(renderer.size.width, renderer.size.height));
        self.ui.draw(&mut scene, &mut self.text_engine);

        match renderer.render(&scene) {
            Ok(RenderStatus::Presented | RenderStatus::Skipped) => {}
            Ok(RenderStatus::Reconfigure) => renderer.resize(renderer.size),
            Err(error) => self.fail(event_loop, error),
        }
    }
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("BE3 Tablet")
            .with_inner_size(LogicalSize::new(INITIAL_WIDTH, INITIAL_HEIGHT));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };
        match Renderer::new(window.clone()) {
            Ok(renderer) => {
                self.window = Some(window);
                self.renderer = Some(renderer);
                self.window.as_ref().unwrap().request_redraw();
            }
            Err(error) => self.fail(event_loop, error),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window.as_ref().map(|window| window.id()) != Some(window_id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let position = Vector::new(position.x as f32, position.y as f32);
                let Some(renderer) = &self.renderer else {
                    return;
                };
                let size = Vector::new(renderer.size.width as f32, renderer.size.height as f32);
                if self.ui.cursor_moved(size, position) {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let Some(renderer) = &self.renderer else {
                    return;
                };
                let size = Vector::new(renderer.size.width as f32, renderer.size.height as f32);
                if self.ui.mouse_pressed(size) {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let Some(renderer) = &self.renderer else {
                    return;
                };
                let size = Vector::new(renderer.size.width as f32, renderer.size.height as f32);
                if self.ui.mouse_released(size) {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::Touch(touch) => {
                let Some(renderer) = &self.renderer else {
                    return;
                };
                let size = Vector::new(renderer.size.width as f32, renderer.size.height as f32);
                let position = Vector::new(touch.location.x as f32, touch.location.y as f32);
                if self.ui.touch_input(size, touch.id, touch.phase, position) {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::RedrawRequested => self.draw(event_loop),
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Page {
    Home,
    Calendar,
    Notes,
    Todo(Feature),
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Feature {
    Calendar,
    Notes,
    Reading,
    Settings,
}

impl Feature {
    const ALL: [Self; 4] = [Self::Calendar, Self::Notes, Self::Reading, Self::Settings];

    fn label(self) -> &'static str {
        match self {
            Self::Calendar => "Calendar",
            Self::Notes => "Notes",
            Self::Reading => "Reading",
            Self::Settings => "Settings",
        }
    }
}

struct TabletUi {
    page: Page,
    calendar: CalendarApp,
    notes: NotesApp,
    cursor_position: Option<Vector<2, f32>>,
    active_touch_id: Option<u64>,
    accepts_mouse_input: bool,
}

impl TabletUi {
    fn new() -> Self {
        Self {
            page: Page::Home,
            calendar: CalendarApp::new(),
            notes: NotesApp::new(),
            cursor_position: None,
            active_touch_id: None,
            accepts_mouse_input: true,
        }
    }

    fn cursor_moved(&mut self, size: Vector<2, f32>, position: Vector<2, f32>) -> bool {
        self.cursor_position = Some(position);
        self.accepts_mouse_input = true;
        if self.active_touch_id.is_some() {
            return false;
        }
        self.pointer_moved(size, position)
    }

    fn mouse_pressed(&mut self, size: Vector<2, f32>) -> bool {
        if self.active_touch_id.is_some() || !self.accepts_mouse_input {
            return false;
        }
        self.cursor_position
            .is_some_and(|position| self.pointer_pressed(size, position))
    }

    fn mouse_released(&mut self, size: Vector<2, f32>) -> bool {
        if self.active_touch_id.is_some() || !self.accepts_mouse_input {
            return false;
        }
        self.cursor_position
            .is_some_and(|position| self.pointer_released(size, position))
    }

    fn touch_input(
        &mut self,
        size: Vector<2, f32>,
        id: u64,
        phase: TouchPhase,
        position: Vector<2, f32>,
    ) -> bool {
        match phase {
            TouchPhase::Started => {
                if self.active_touch_id.is_some() {
                    return false;
                }
                self.active_touch_id = Some(id);
                self.accepts_mouse_input = false;
                self.pointer_pressed(size, position)
            }
            TouchPhase::Moved => {
                if self.active_touch_id != Some(id) {
                    return false;
                }
                self.pointer_moved(size, position)
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if self.active_touch_id != Some(id) {
                    return false;
                }
                self.active_touch_id = None;
                self.pointer_released(size, position)
            }
        }
    }

    fn pointer_pressed(&mut self, size: Vector<2, f32>, position: Vector<2, f32>) -> bool {
        match self.page {
            Page::Notes => self.notes.pointer_pressed(size, position),
            Page::Home | Page::Calendar | Page::Todo(_) => false,
        }
    }

    fn pointer_moved(&mut self, size: Vector<2, f32>, position: Vector<2, f32>) -> bool {
        match self.page {
            Page::Notes => self.notes.pointer_moved(size, position),
            Page::Home | Page::Calendar | Page::Todo(_) => false,
        }
    }

    fn pointer_released(&mut self, size: Vector<2, f32>, position: Vector<2, f32>) -> bool {
        if matches!(self.page, Page::Notes) && self.notes.pointer_released(size, position) {
            return true;
        }
        if home_button_rect().contains(position) {
            return self.set_page(Page::Home);
        }

        match self.page {
            Page::Calendar => return self.calendar.click(size, position),
            Page::Notes => return false,
            Page::Todo(_) => return false,
            Page::Home => {}
        }

        Feature::ALL
            .into_iter()
            .find(|feature| quadrant_rect(size, *feature).contains(position))
            .is_some_and(|feature| {
                self.set_page(match feature {
                    Feature::Calendar => Page::Calendar,
                    Feature::Notes => Page::Notes,
                    _ => Page::Todo(feature),
                })
            })
    }

    fn set_page(&mut self, page: Page) -> bool {
        if self.page == page {
            return false;
        }
        self.page = page;
        true
    }

    fn draw(&self, scene: &mut Scene, text: &mut TextEngine) {
        let size = scene.size();
        scene.push_rect(Rect::new(Vector::new(0.0, 0.0), size), Color::WHITE);
        self.draw_status_bar(scene, text);
        match self.page {
            Page::Home => self.draw_home(scene, text, size),
            Page::Calendar => self.calendar.draw(scene, text, size),
            Page::Notes => self.notes.draw(scene, text, size),
            Page::Todo(feature) => self.draw_todo(scene, text, size, feature),
        }
    }

    fn draw_status_bar(&self, scene: &mut Scene, text: &mut TextEngine) {
        scene.push_rect(
            Rect::new(Vector::new(0.0, 0.0), Vector::new(scene.size()[0], 1.0)),
            Color::BLACK,
        );
        scene.push_rect(
            Rect::new(
                Vector::new(0.0, STATUS_BAR_HEIGHT - 1.0),
                Vector::new(scene.size()[0], 1.0),
            ),
            Color::BLACK,
        );
        let home = home_button_rect();
        scene.stroke_rect(home, 2.0, Color::BLACK);
        text.draw(
            scene,
            Vector::new(home.position[0] + 22.0, 16.0),
            "Home",
            Color::BLACK,
        );
        text.draw(
            scene,
            Vector::new(scene.size()[0] - 156.0, 16.0),
            "BE3 Tablet",
            Color::BLACK,
        );
    }

    fn draw_home(&self, scene: &mut Scene, text: &mut TextEngine, size: Vector<2, f32>) {
        for feature in Feature::ALL {
            let rect = quadrant_rect(size, feature);
            scene.stroke_rect(rect, 3.0, Color::BLACK);
            let label = feature.label();
            let label_x = rect.position[0] + 28.0;
            let label_y = rect.position[1] + rect.size[1] * 0.5 - 9.0;
            text.draw(scene, Vector::new(label_x, label_y), label, Color::BLACK);
        }
    }

    fn draw_todo(
        &self,
        scene: &mut Scene,
        text: &mut TextEngine,
        size: Vector<2, f32>,
        feature: Feature,
    ) {
        let body = Rect::new(
            Vector::new(OUTER_MARGIN, STATUS_BAR_HEIGHT + OUTER_MARGIN),
            Vector::new(
                (size[0] - OUTER_MARGIN * 2.0).max(1.0),
                (size[1] - STATUS_BAR_HEIGHT - OUTER_MARGIN * 2.0).max(1.0),
            ),
        );
        scene.stroke_rect(body, 3.0, Color::BLACK);
        text.draw(
            scene,
            Vector::new(body.position[0] + 28.0, body.position[1] + 34.0),
            feature.label(),
            Color::BLACK,
        );
        text.draw(
            scene,
            Vector::new(body.position[0] + 28.0, body.position[1] + 88.0),
            "TODO",
            Color::BLACK,
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Vector<const N: usize, T> {
    values: [T; N],
}

impl<T> Vector<2, T> {
    pub(crate) const fn new(x: T, y: T) -> Self {
        Self { values: [x, y] }
    }
}

impl<const N: usize, T> Index<usize> for Vector<N, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.values[index]
    }
}

impl<const N: usize, T: Add<Output = T> + Copy> Add for Vector<N, T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            values: std::array::from_fn(|index| self[index] + rhs[index]),
        }
    }
}

impl<const N: usize, T: Sub<Output = T> + Copy> Sub for Vector<N, T> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            values: std::array::from_fn(|index| self[index] - rhs[index]),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Rect {
    pub(crate) position: Vector<2, f32>,
    pub(crate) size: Vector<2, f32>,
}

impl Rect {
    pub(crate) const fn new(position: Vector<2, f32>, size: Vector<2, f32>) -> Self {
        Self { position, size }
    }

    pub(crate) fn contains(self, position: Vector<2, f32>) -> bool {
        position[0] >= self.position[0]
            && position[1] >= self.position[1]
            && position[0] < self.position[0] + self.size[0]
            && position[1] < self.position[1] + self.size[1]
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Color(u32);

impl Color {
    pub(crate) const BLACK: Self = Self::rgb(0, 0, 0);
    pub(crate) const GRAY: Self = Self::rgb(128, 128, 128);
    pub(crate) const WHITE: Self = Self::rgb(255, 255, 255);

    pub(crate) const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(((red as u32) << 16) | ((green as u32) << 8) | blue as u32)
    }

    fn as_f32(self) -> [f32; 4] {
        [
            ((self.0 >> 16) & 0xff) as f32 / 255.0,
            ((self.0 >> 8) & 0xff) as f32 / 255.0,
            (self.0 & 0xff) as f32 / 255.0,
            1.0,
        ]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coord: [f32; 2],
    color: [f32; 4],
}

pub(crate) struct Scene {
    size: Vector<2, f32>,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    atlas: Vec<u8>,
    atlas_x: u32,
    atlas_y: u32,
    atlas_row_height: u32,
}

impl Scene {
    fn new(size: Vector<2, u32>) -> Self {
        Self {
            size: Vector::new(size[0].max(1) as f32, size[1].max(1) as f32),
            vertices: Vec::new(),
            indices: Vec::new(),
            atlas: vec![0; (ATLAS_SIZE * ATLAS_SIZE) as usize],
            atlas_x: 0,
            atlas_y: 0,
            atlas_row_height: 0,
        }
    }

    fn size(&self) -> Vector<2, f32> {
        self.size
    }

    pub(crate) fn add_glyph(
        &mut self,
        size: Vector<2, u32>,
        pixels: &[u8],
    ) -> Option<([f32; 2], [f32; 2])> {
        self.add_bitmap(size, pixels)
    }

    pub(crate) fn add_bitmap(
        &mut self,
        size: Vector<2, u32>,
        pixels: &[u8],
    ) -> Option<([f32; 2], [f32; 2])> {
        let width = size[0];
        let height = size[1];
        if width == 0 || height == 0 || width >= ATLAS_SIZE || height >= ATLAS_SIZE {
            return None;
        }
        if self.atlas_x + width + 1 > ATLAS_SIZE {
            self.atlas_x = 0;
            self.atlas_y += self.atlas_row_height + 1;
            self.atlas_row_height = 0;
        }
        if self.atlas_y + height > ATLAS_SIZE {
            return None;
        }
        let x = self.atlas_x;
        let y = self.atlas_y;
        for row in 0..height {
            let destination = ((y + row) * ATLAS_SIZE + x) as usize;
            let source = (row * width) as usize;
            self.atlas[destination..destination + width as usize]
                .copy_from_slice(&pixels[source..source + width as usize]);
        }
        self.atlas_x += width + 1;
        self.atlas_row_height = self.atlas_row_height.max(height);
        Some((
            [x as f32 / ATLAS_SIZE as f32, y as f32 / ATLAS_SIZE as f32],
            [
                (x + width) as f32 / ATLAS_SIZE as f32,
                (y + height) as f32 / ATLAS_SIZE as f32,
            ],
        ))
    }

    pub(crate) fn push_quad(
        &mut self,
        rect: Rect,
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        color: Color,
    ) {
        let x0 = rect.position[0] / self.size[0] * 2.0 - 1.0;
        let x1 = (rect.position[0] + rect.size[0]) / self.size[0] * 2.0 - 1.0;
        let y0 = 1.0 - rect.position[1] / self.size[1] * 2.0;
        let y1 = 1.0 - (rect.position[1] + rect.size[1]) / self.size[1] * 2.0;
        let base = self.vertices.len() as u32;
        let color = color.as_f32();
        self.vertices.extend_from_slice(&[
            Vertex {
                position: [x0, y0],
                tex_coord: [uv_min[0], uv_min[1]],
                color,
            },
            Vertex {
                position: [x1, y0],
                tex_coord: [uv_max[0], uv_min[1]],
                color,
            },
            Vertex {
                position: [x1, y1],
                tex_coord: [uv_max[0], uv_max[1]],
                color,
            },
            Vertex {
                position: [x0, y1],
                tex_coord: [uv_min[0], uv_max[1]],
                color,
            },
        ]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub(crate) fn push_rect(&mut self, rect: Rect, color: Color) {
        self.push_quad(rect, [-1.0, -1.0], [-1.0, -1.0], color);
    }

    pub(crate) fn stroke_rect(&mut self, rect: Rect, thickness: f32, color: Color) {
        let thickness = thickness
            .min(rect.size[0].max(0.0) * 0.5)
            .min(rect.size[1].max(0.0) * 0.5);
        self.push_rect(
            Rect::new(rect.position, Vector::new(rect.size[0], thickness)),
            color,
        );
        self.push_rect(
            Rect::new(
                Vector::new(
                    rect.position[0],
                    rect.position[1] + rect.size[1] - thickness,
                ),
                Vector::new(rect.size[0], thickness),
            ),
            color,
        );
        self.push_rect(
            Rect::new(rect.position, Vector::new(thickness, rect.size[1])),
            color,
        );
        self.push_rect(
            Rect::new(
                Vector::new(
                    rect.position[0] + rect.size[0] - thickness,
                    rect.position[1],
                ),
                Vector::new(thickness, rect.size[1]),
            ),
            color,
        );
    }
}

fn home_button_rect() -> Rect {
    Rect::new(
        Vector::new(OUTER_MARGIN, 10.0),
        Vector::new(HOME_BUTTON_WIDTH, STATUS_BAR_HEIGHT - 20.0),
    )
}

fn quadrant_rect(size: Vector<2, f32>, feature: Feature) -> Rect {
    let content_x = OUTER_MARGIN;
    let content_y = STATUS_BAR_HEIGHT + OUTER_MARGIN;
    let content_width = (size[0] - OUTER_MARGIN * 2.0).max(1.0);
    let content_height = (size[1] - STATUS_BAR_HEIGHT - OUTER_MARGIN * 2.0).max(1.0);
    let button_width = ((content_width - BUTTON_GAP) * 0.5).max(1.0);
    let button_height = ((content_height - BUTTON_GAP) * 0.5).max(1.0);
    let (column, row) = match feature {
        Feature::Calendar => (0.0, 0.0),
        Feature::Notes => (1.0, 0.0),
        Feature::Reading => (0.0, 1.0),
        Feature::Settings => (1.0, 1.0),
    };
    Rect::new(
        Vector::new(
            content_x + column * (button_width + BUTTON_GAP),
            content_y + row * (button_height + BUTTON_GAP),
        ),
        Vector::new(button_width, button_height),
    )
}

enum RenderStatus {
    Presented,
    Reconfigure,
    Skipped,
}

#[path = "renderer/vulkan.rs"]
mod vulkan;
use vulkan::Renderer;

#[cfg(test)]
#[path = "renderer/tests.rs"]
mod tests;
