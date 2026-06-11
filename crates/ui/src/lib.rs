use bytemuck::{Pod, Zeroable};
use freetype::freetype as ft;
use harfbuzz_rs::{shape, Face as HbFace, Font as HbFont, Tag, UnicodeBuffer};
use once_cell::sync::OnceCell;
use std::ffi::CString;
use std::path::Path;
use std::ptr;
use std::sync::Arc;
use unicode_script::{Script, UnicodeScript};
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeRecommendation {
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl SizeRecommendation {
    pub const fn new(width: Option<f32>, height: Option<f32>) -> Self {
        Self { width, height }
    }

    pub const fn exact(width: f32, height: f32) -> Self {
        Self {
            width: Some(width),
            height: Some(height),
        }
    }

    fn main(self, axis: Axis) -> Option<f32> {
        match axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
        }
    }

    fn with_main(self, axis: Axis, value: Option<f32>) -> Self {
        match axis {
            Axis::Horizontal => Self::new(value, self.height),
            Axis::Vertical => Self::new(self.width, value),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    fn main(self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
        }
    }

    fn cross(self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.height,
            Axis::Vertical => self.width,
        }
    }

    fn from_axes(axis: Axis, main: f32, cross: f32) -> Self {
        match axis {
            Axis::Horizontal => Self::new(main, cross),
            Axis::Vertical => Self::new(cross, main),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn size(self) -> Size {
        Size::new(self.width, self.height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sizing {
    Intrinsic,
    Fr(f32),
}

impl Sizing {
    pub const fn fr(value: f32) -> Self {
        Self::Fr(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeSource {
    Parent,
    Child,
    Zero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(u32);

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
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

#[derive(Clone, Debug)]
pub struct Component {
    kind: Kind,
    rect: Rect,
}

#[derive(Clone, Debug)]
enum Kind {
    Sized(SizedComponent),
    Fill(Fill),
    Text(Text),
    Button(Box<Component>),
    List(List),
    Scrollable(Scrollable),
}

#[derive(Clone, Debug)]
pub struct SizedComponent {
    x: SizeSource,
    y: SizeSource,
    child: Option<Box<Component>>,
}

#[derive(Clone, Debug)]
pub struct Fill {
    color: Color,
    child: Box<Component>,
}

#[derive(Clone, Debug)]
pub struct Text {
    value: String,
}

#[derive(Clone, Debug)]
pub struct List {
    axis: Axis,
    children: Vec<ListChild>,
}

#[derive(Clone, Debug)]
struct ListChild {
    sizing: Sizing,
    component: Component,
}

#[derive(Clone, Debug)]
pub struct Scrollable {
    axis: Axis,
    child: Box<Component>,
}

impl Component {
    pub fn sized(x: SizeSource, y: SizeSource, child: Option<Component>) -> Self {
        Self::new(Kind::Sized(SizedComponent {
            x,
            y,
            child: child.map(Box::new),
        }))
    }

    pub fn fill(color: Color, child: Component) -> Self {
        Self::new(Kind::Fill(Fill {
            color,
            child: Box::new(child),
        }))
    }

    pub fn text(value: impl Into<String>) -> Self {
        Self::new(Kind::Text(Text {
            value: value.into(),
        }))
    }

    pub fn button(child: Component) -> Self {
        Self::new(Kind::Button(Box::new(child)))
    }

    pub fn list<const N: usize>(axis: Axis, children: [(Sizing, Component); N]) -> Self {
        Self::new(Kind::List(List {
            axis,
            children: children
                .into_iter()
                .map(|(sizing, component)| ListChild { sizing, component })
                .collect(),
        }))
    }

    pub fn scrollable(axis: Axis, child: Component) -> Self {
        Self::new(Kind::Scrollable(Scrollable {
            axis,
            child: Box::new(child),
        }))
    }

    pub fn layout(&mut self, recommendation: SizeRecommendation) -> Size {
        let size = match &mut self.kind {
            Kind::Sized(sized) => sized.layout(recommendation),
            Kind::Fill(fill) => fill.child.layout(recommendation),
            Kind::Text(text) => text.layout(),
            Kind::Button(child) => child.layout(recommendation),
            Kind::List(list) => list.layout(recommendation),
            Kind::Scrollable(scrollable) => scrollable.layout(recommendation),
        };
        self.rect.width = size.width;
        self.rect.height = size.height;
        size
    }

    pub fn place(&mut self, rect: Rect) {
        self.rect = rect;
        match &mut self.kind {
            Kind::Sized(sized) => sized.place(),
            Kind::Text(_) => {}
            Kind::Fill(fill) => fill
                .child
                .place(Rect::new(0.0, 0.0, rect.width, rect.height)),
            Kind::Button(child) => child.place(Rect::new(0.0, 0.0, rect.width, rect.height)),
            Kind::List(list) => list.place(rect.size()),
            Kind::Scrollable(scrollable) => scrollable.place(rect.size()),
        }
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    fn new(kind: Kind) -> Self {
        Self {
            kind,
            rect: Rect::default(),
        }
    }

    fn paint(&self, scene: &mut Scene, offset_x: f32, offset_y: f32) {
        let x = offset_x + self.rect.x;
        let y = offset_y + self.rect.y;
        match &self.kind {
            Kind::Sized(sized) => {
                if let Some(child) = &sized.child {
                    child.paint(scene, x, y);
                }
            }
            Kind::Fill(fill) => {
                scene.fill_rect(
                    Rect::new(x, y, self.rect.width, self.rect.height),
                    fill.color,
                );
                fill.child.paint(scene, x, y);
            }
            Kind::Text(text) => scene.draw_text(x, y + 2.0, &text.value, Color::BLACK),
            Kind::Button(child) => {
                scene.fill_rect(
                    Rect::new(x, y, self.rect.width, self.rect.height),
                    Color::rgb(0xf2, 0xf2, 0xf2),
                );
                scene.stroke_rect(
                    Rect::new(x, y, self.rect.width, self.rect.height),
                    Color::BLACK,
                );
                child.paint(scene, x, y);
            }
            Kind::List(list) => {
                for child in &list.children {
                    child.component.paint(scene, x, y);
                }
            }
            Kind::Scrollable(scrollable) => {
                scrollable.child.paint(scene, x, y);
                let bar_color = Color::rgb(0xc0, 0xc0, 0xc0);
                match scrollable.axis {
                    Axis::Vertical => scene.fill_rect(
                        Rect::new(
                            x + self.rect.width - SCROLLBAR_SIZE,
                            y,
                            SCROLLBAR_SIZE,
                            self.rect.height,
                        ),
                        bar_color,
                    ),
                    Axis::Horizontal => scene.fill_rect(
                        Rect::new(
                            x,
                            y + self.rect.height - SCROLLBAR_SIZE,
                            self.rect.width,
                            SCROLLBAR_SIZE,
                        ),
                        bar_color,
                    ),
                }
            }
        }
    }
}

impl SizedComponent {
    fn layout(&mut self, recommendation: SizeRecommendation) -> Size {
        let child_size = self
            .child
            .as_mut()
            .map(|child| child.layout(recommendation))
            .unwrap_or(Size::ZERO);
        Size::new(
            Self::axis_size(self.x, recommendation.width, child_size.width),
            Self::axis_size(self.y, recommendation.height, child_size.height),
        )
    }

    fn axis_size(source: SizeSource, parent: Option<f32>, child: f32) -> f32 {
        match source {
            SizeSource::Parent => parent.unwrap_or(0.0),
            SizeSource::Child => child,
            SizeSource::Zero => 0.0,
        }
    }

    fn place(&mut self) {
        if let Some(child) = &mut self.child {
            let size = child.rect.size();
            child.place(Rect::new(0.0, 0.0, size.width, size.height));
        }
    }
}

impl Text {
    fn layout(&self) -> Size {
        TextEngine::new()
            .map(|engine| engine.measure(&self.value))
            .unwrap_or_else(|| Size::new(self.value.chars().count() as f32 * 10.0, 20.0))
    }
}

impl List {
    fn layout(&mut self, recommendation: SizeRecommendation) -> Size {
        let axis = self.axis;
        let mut intrinsic_main: f32 = 0.0;
        let mut max_cross: f32 = 0.0;
        let mut fr_total: f32 = 0.0;

        for child in &mut self.children {
            match child.sizing {
                Sizing::Intrinsic => {
                    let size = child.component.layout(recommendation);
                    intrinsic_main += size.main(axis);
                    max_cross = max_cross.max(size.cross(axis));
                }
                Sizing::Fr(value) => fr_total += value.max(0.0),
            }
        }

        let remaining = recommendation
            .main(axis)
            .map(|main| (main - intrinsic_main).max(0.0));
        let mut fr_main: f32 = 0.0;

        for child in &mut self.children {
            if let Sizing::Fr(value) = child.sizing {
                let share = remaining.map(|remaining| {
                    if fr_total > 0.0 {
                        remaining * value.max(0.0) / fr_total
                    } else {
                        0.0
                    }
                });
                let size = child
                    .component
                    .layout(recommendation.with_main(axis, share));
                fr_main += size.main(axis);
                max_cross = max_cross.max(size.cross(axis));
            }
        }

        Size::from_axes(axis, intrinsic_main + fr_main, max_cross)
    }

    fn place(&mut self, size: Size) {
        let axis = self.axis;
        let mut cursor = 0.0;
        for child in &mut self.children {
            let child_size = child.component.rect.size();
            let rect = match axis {
                Axis::Horizontal => Rect::new(cursor, 0.0, child_size.width, size.height),
                Axis::Vertical => Rect::new(0.0, cursor, size.width, child_size.height),
            };
            child.component.place(rect);
            cursor += child_size.main(axis);
        }
    }
}

const SCROLLBAR_SIZE: f32 = 20.0;

impl Scrollable {
    fn layout(&mut self, recommendation: SizeRecommendation) -> Size {
        let viewport = Size::new(
            recommendation.width.unwrap_or(0.0),
            recommendation.height.unwrap_or(0.0),
        );
        let child_recommendation = match self.axis {
            Axis::Vertical => SizeRecommendation::new(
                Some((viewport.width - SCROLLBAR_SIZE).max(0.0)),
                Some(viewport.height),
            ),
            Axis::Horizontal => SizeRecommendation::new(
                Some(viewport.width),
                Some((viewport.height - SCROLLBAR_SIZE).max(0.0)),
            ),
        };
        self.child.layout(child_recommendation);
        viewport
    }

    fn place(&mut self, size: Size) {
        let child_size = self.child.rect.size();
        self.child
            .place(Rect::new(0.0, 0.0, child_size.width, child_size.height));
        match self.axis {
            Axis::Vertical => {
                self.child.rect.width = self.child.rect.width.min(size.width - SCROLLBAR_SIZE);
            }
            Axis::Horizontal => {
                self.child.rect.height = self.child.rect.height.min(size.height - SCROLLBAR_SIZE);
            }
        }
    }
}

pub struct UiWindow {
    title: String,
    width: u32,
    height: u32,
}

impl UiWindow {
    pub fn new(
        title: &str,
        width: usize,
        height: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let width = u32::try_from(width)?;
        let height = u32::try_from(height)?;
        if width == 0 || height == 0 {
            return Err("window dimensions must be non-zero".into());
        }
        Ok(Self {
            title: title.to_owned(),
            width,
            height,
        })
    }

    pub fn run(
        &mut self,
        root: Component,
        initial_recommendation: SizeRecommendation,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::new()?;
        let mut app = UiApplication::new(
            self.title.clone(),
            PhysicalSize::new(self.width, self.height),
            root,
            initial_recommendation,
        );
        event_loop.run_app(&mut app)?;
        if let Some(error) = app.error {
            return Err(error.into());
        }
        Ok(())
    }
}

struct UiApplication {
    title: String,
    initial_size: PhysicalSize<u32>,
    root: Component,
    recommendation: SizeRecommendation,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    error: Option<String>,
}

impl UiApplication {
    fn new(
        title: String,
        initial_size: PhysicalSize<u32>,
        root: Component,
        recommendation: SizeRecommendation,
    ) -> Self {
        Self {
            title,
            initial_size,
            root,
            recommendation,
            window: None,
            renderer: None,
            error: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl ToString) {
        self.error = Some(error.to_string());
        event_loop.exit();
    }
}

impl ApplicationHandler for UiApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(&self.title)
            .with_inner_size(LogicalSize::new(
                self.initial_size.width,
                self.initial_size.height,
            ));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };
        match pollster::block_on(Renderer::new(window.clone())) {
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
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && event.logical_key == Key::Named(NamedKey::Escape) =>
            {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let Some(renderer) = &mut self.renderer else {
                    return;
                };
                let size = self.root.layout(self.recommendation);
                self.root
                    .place(Rect::new(0.0, 0.0, size.width, size.height));
                let mut scene = Scene::new(renderer.size.width, renderer.size.height);
                self.root.paint(&mut scene, 0.0, 0.0);
                let result = renderer.render(&scene);
                match result {
                    Ok(RenderStatus::Presented | RenderStatus::Skipped) => {}
                    Ok(RenderStatus::Reconfigure) => renderer.resize(renderer.size),
                    Err(error) => self.fail(event_loop, error),
                }
            }
            _ => {}
        }
    }
}

const ATLAS_SIZE: u32 = 1024;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coord: [f32; 2],
    color: [f32; 4],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

struct Scene {
    width: f32,
    height: f32,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    atlas: Vec<u8>,
    atlas_x: u32,
    atlas_y: u32,
    atlas_row_height: u32,
}

impl Scene {
    fn new(width: u32, height: u32) -> Self {
        let mut atlas = vec![0; (ATLAS_SIZE * ATLAS_SIZE) as usize];
        atlas[0] = 255;
        Self {
            width: width.max(1) as f32,
            height: height.max(1) as f32,
            vertices: Vec::new(),
            indices: Vec::new(),
            atlas,
            atlas_x: 1,
            atlas_y: 0,
            atlas_row_height: 1,
        }
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.push_quad(
            rect,
            [0.5 / ATLAS_SIZE as f32; 2],
            [0.5 / ATLAS_SIZE as f32; 2],
            color,
        );
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color) {
        self.fill_rect(Rect::new(rect.x, rect.y, rect.width, 1.0), color);
        self.fill_rect(
            Rect::new(rect.x, rect.y + rect.height - 1.0, rect.width, 1.0),
            color,
        );
        self.fill_rect(Rect::new(rect.x, rect.y, 1.0, rect.height), color);
        self.fill_rect(
            Rect::new(rect.x + rect.width - 1.0, rect.y, 1.0, rect.height),
            color,
        );
    }

    fn draw_text(&mut self, x: f32, y: f32, value: &str, color: Color) {
        if let Some(mut engine) = TextEngine::new() {
            engine.draw(self, x, y, value, color);
        }
    }

    fn add_glyph(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Option<([f32; 2], [f32; 2])> {
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

    fn push_quad(&mut self, rect: Rect, uv_min: [f32; 2], uv_max: [f32; 2], color: Color) {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }
        let x0 = rect.x / self.width * 2.0 - 1.0;
        let x1 = (rect.x + rect.width) / self.width * 2.0 - 1.0;
        let y0 = 1.0 - rect.y / self.height * 2.0;
        let y1 = 1.0 - (rect.y + rect.height) / self.height * 2.0;
        let color = color.as_f32();
        let base = self.vertices.len() as u32;
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
}

struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    pipeline: wgpu::RenderPipeline,
    atlas_texture: wgpu::Texture,
    atlas_bind_group: wgpu::BindGroup,
}

enum RenderStatus {
    Presented,
    Reconfigure,
    Skipped,
}

impl Renderer {
    async fn new(window: Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("ui device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await?;
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface is not supported by the selected adapter")?;
        surface.configure(&device, &config);

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui glyph atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ui glyph sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui atlas layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui atlas bind group"),
            layout: &atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("ui.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui pipeline layout"),
            bind_group_layouts: &[Some(&atlas_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Vertex::layout()],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
            atlas_texture,
            atlas_bind_group,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.size = size;
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self, scene: &Scene) -> Result<RenderStatus, String> {
        if self.size.width == 0 || self.size.height == 0 {
            return Ok(RenderStatus::Skipped);
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &scene.atlas,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ATLAS_SIZE),
                rows_per_image: Some(ATLAS_SIZE),
            },
            wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
        );
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ui vertices"),
                contents: bytemuck::cast_slice(&scene.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ui indices"),
                contents: bytemuck::cast_slice(&scene.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        let (frame, status) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, RenderStatus::Presented),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, RenderStatus::Reconfigure),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(RenderStatus::Skipped);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                return Ok(RenderStatus::Reconfigure);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("wgpu surface texture validation failed".to_owned());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ui command encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0xee as f64 / 255.0,
                            g: 0xee as f64 / 255.0,
                            b: 0xee as f64 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if !scene.indices.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.atlas_bind_group, &[]);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..scene.indices.len() as u32, 0, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(status)
    }
}

const TEXT_PIXEL_SIZE: u32 = 18;
const TEXT_SCALE: i32 = (TEXT_PIXEL_SIZE as i32) * 64;

struct TextEngine {
    library: ft::FT_Library,
    fonts: Vec<FontFace>,
}

struct FontFace {
    face: ft::FT_Face,
    font_path: &'static str,
}

#[derive(Clone, Copy)]
struct ShapedGlyph {
    font_index: usize,
    id: u32,
    x_advance: f32,
    x_offset: f32,
    y_offset: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TextRun<'a> {
    value: &'a str,
    script: Option<Script>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FontRun<'a> {
    value: &'a str,
    script: Option<Script>,
    font_index: usize,
}

impl TextEngine {
    fn new() -> Option<Self> {
        let mut library = ptr::null_mut();
        unsafe {
            if ft::FT_Init_FreeType(&mut library) != 0 {
                return None;
            }
        }

        let fonts = font_paths()
            .iter()
            .filter_map(|font_path| {
                let font_path_c = CString::new(*font_path).ok()?;
                let mut face = ptr::null_mut();
                unsafe {
                    if ft::FT_New_Face(library, font_path_c.as_ptr(), 0, &mut face) != 0 {
                        return None;
                    }
                    if ft::FT_Set_Pixel_Sizes(face, 0, TEXT_PIXEL_SIZE) != 0 {
                        ft::FT_Done_Face(face);
                        return None;
                    }
                }
                Some(FontFace { face, font_path })
            })
            .collect::<Vec<_>>();

        if fonts.is_empty() {
            unsafe {
                ft::FT_Done_FreeType(library);
            }
            None
        } else {
            Some(Self { library, fonts })
        }
    }

    fn measure(&self, value: &str) -> Size {
        let width = self
            .shape(value)
            .into_iter()
            .map(|glyph| glyph.x_advance)
            .sum::<f32>()
            .ceil();
        Size::new(width, self.line_height())
    }

    fn draw(&mut self, scene: &mut Scene, x: f32, y: f32, value: &str, color: Color) {
        let baseline = y + self.ascender();
        let mut pen_x = x;

        for glyph in self.shape(value) {
            let face = self.fonts[glyph.font_index].face;
            unsafe {
                if ft::FT_Load_Glyph(face, glyph.id, ft::FT_LOAD_DEFAULT as i32) == 0 {
                    let slot = (*face).glyph;
                    if ft::FT_Render_Glyph(slot, ft::FT_Render_Mode::FT_RENDER_MODE_NORMAL) == 0 {
                        let bitmap_x = pen_x + glyph.x_offset + (*slot).bitmap_left as f32;
                        let bitmap_y = baseline - glyph.y_offset - (*slot).bitmap_top as f32;
                        paint_glyph_bitmap(scene, bitmap_x, bitmap_y, &(*slot).bitmap, color);
                    }
                }
            }

            pen_x += glyph.x_advance;
        }
    }

    fn shape(&self, value: &str) -> Vec<ShapedGlyph> {
        self.font_runs(value)
            .into_iter()
            .flat_map(|run| {
                let hb_face = match HbFace::from_file(self.fonts[run.font_index].font_path, 0) {
                    Ok(face) => face,
                    Err(_) => return Vec::new(),
                };
                let mut hb_font = HbFont::new(hb_face);
                hb_font.set_scale(TEXT_SCALE, TEXT_SCALE);
                hb_font.set_ppem(TEXT_PIXEL_SIZE, TEXT_PIXEL_SIZE);
                let mut buffer = UnicodeBuffer::new().add_str(run.value);
                if let Some(script) = run.script {
                    let tag = script.as_iso15924_tag().to_be_bytes();
                    buffer = buffer.set_script(Tag::new(
                        tag[0] as char,
                        tag[1] as char,
                        tag[2] as char,
                        tag[3] as char,
                    ));
                }
                let output = shape(&hb_font, buffer.guess_segment_properties(), &[]);
                output
                    .get_glyph_infos()
                    .iter()
                    .zip(output.get_glyph_positions())
                    .map(|(info, position)| ShapedGlyph {
                        font_index: run.font_index,
                        id: info.codepoint,
                        x_advance: position.x_advance as f32 / 64.0,
                        x_offset: position.x_offset as f32 / 64.0,
                        y_offset: position.y_offset as f32 / 64.0,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn font_runs<'a>(&self, value: &'a str) -> Vec<FontRun<'a>> {
        script_runs(value)
            .into_iter()
            .flat_map(|run| split_font_runs(run, |character| self.font_index_for(character)))
            .collect()
    }

    fn font_index_for(&self, character: char) -> Option<usize> {
        self.fonts.iter().position(|font| unsafe {
            ft::FT_Get_Char_Index(font.face, character as ft::FT_ULong) != 0
        })
    }

    fn ascender(&self) -> f32 {
        self.fonts
            .iter()
            .map(|font| face_ascender(font.face))
            .fold(TEXT_PIXEL_SIZE as f32, f32::max)
    }

    fn line_height(&self) -> f32 {
        self.fonts
            .iter()
            .map(|font| face_line_height(font.face))
            .fold((TEXT_PIXEL_SIZE as f32 * 1.2).ceil(), f32::max)
    }
}

fn split_font_runs(
    run: TextRun<'_>,
    mut font_index_for: impl FnMut(char) -> Option<usize>,
) -> Vec<FontRun<'_>> {
    let mut runs = Vec::new();
    let mut run_start = 0;
    let mut current_font = None;

    for (index, character) in run.value.char_indices() {
        let font_index = font_index_for(character).or(current_font).unwrap_or(0);
        match current_font {
            None => current_font = Some(font_index),
            Some(current) if current == font_index => {}
            Some(current) => {
                runs.push(FontRun {
                    value: &run.value[run_start..index],
                    script: run.script,
                    font_index: current,
                });
                run_start = index;
                current_font = Some(font_index);
            }
        }
    }

    if let Some(font_index) = current_font {
        runs.push(FontRun {
            value: &run.value[run_start..],
            script: run.script,
            font_index,
        });
    }

    runs
}

fn face_ascender(face: ft::FT_Face) -> f32 {
    unsafe {
        let size = (*face).size;
        if size.is_null() {
            TEXT_PIXEL_SIZE as f32
        } else {
            (*size).metrics.ascender as f32 / 64.0
        }
    }
}

fn face_line_height(face: ft::FT_Face) -> f32 {
    unsafe {
        let size = (*face).size;
        if size.is_null() {
            (TEXT_PIXEL_SIZE as f32 * 1.2).ceil()
        } else {
            ((*size).metrics.height as f32 / 64.0).ceil()
        }
    }
}

fn script_runs(value: &str) -> Vec<TextRun<'_>> {
    let mut runs = Vec::new();
    let mut run_start = 0;
    let mut current = None;

    for (index, character) in value.char_indices() {
        let script = character.script();
        if matches!(script, Script::Common | Script::Inherited | Script::Unknown) {
            continue;
        }

        match current {
            None => current = Some(script),
            Some(current_script) if current_script == script => {}
            Some(_) => {
                runs.push(TextRun {
                    value: &value[run_start..index],
                    script: current,
                });
                run_start = index;
                current = Some(script);
            }
        }
    }

    if !value.is_empty() {
        runs.push(TextRun {
            value: &value[run_start..],
            script: current,
        });
    }

    runs
}

impl Drop for TextEngine {
    fn drop(&mut self) {
        unsafe {
            for font in &self.fonts {
                if !font.face.is_null() {
                    ft::FT_Done_Face(font.face);
                }
            }
            if !self.library.is_null() {
                ft::FT_Done_FreeType(self.library);
            }
        }
    }
}

fn paint_glyph_bitmap(scene: &mut Scene, x: f32, y: f32, bitmap: &ft::FT_Bitmap, color: Color) {
    let width = bitmap.width as i32;
    let rows = bitmap.rows as i32;
    let pitch = bitmap.pitch.unsigned_abs() as usize;
    let byte_len = pitch * rows.max(0) as usize;
    if bitmap.buffer.is_null() || width <= 0 || rows <= 0 || byte_len == 0 {
        return;
    }
    let buffer = unsafe { std::slice::from_raw_parts(bitmap.buffer, byte_len) };
    let mut pixels = vec![0; (width * rows) as usize];
    for row in 0..rows {
        let source_row = if bitmap.pitch >= 0 {
            row as usize
        } else {
            (rows - 1 - row) as usize
        };
        let row_start = source_row * pitch;
        for col in 0..width {
            let index = row_start + col as usize;
            if let Some(alpha) = buffer.get(index).copied() {
                pixels[(row * width + col) as usize] = alpha;
            }
        }
    }
    if let Some((uv_min, uv_max)) = scene.add_glyph(width as u32, rows as u32, &pixels) {
        scene.push_quad(
            Rect::new(x.round(), y.round(), width as f32, rows as f32),
            uv_min,
            uv_max,
            color,
        );
    }
}

fn font_paths() -> &'static Vec<&'static str> {
    static FONT_PATHS: OnceCell<Vec<&'static str>> = OnceCell::new();
    FONT_PATHS.get_or_init(|| {
        FONT_CANDIDATES
            .iter()
            .copied()
            .filter(|path| Path::new(path).exists())
            .collect()
    })
}

const FONT_CANDIDATES: &[&str] = &[
    "C:\\Windows\\Fonts\\verdana.ttf",
    "/System/Library/Fonts/Supplemental/Verdana.ttf",
    "/Library/Fonts/Verdana.ttf",
    "/usr/share/fonts/truetype/msttcorefonts/Verdana.ttf",
    "/usr/share/fonts/truetype/msttcorefonts/verdana.ttf",
    "C:\\Windows\\Fonts\\segoeui.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
    "C:\\Windows\\Fonts\\msyh.ttc",
    "C:\\Windows\\Fonts\\meiryo.ttc",
    "C:\\Windows\\Fonts\\malgun.ttf",
    "C:\\Windows\\Fonts\\Nirmala.ttf",
    "C:\\Windows\\Fonts\\seguisym.ttf",
    "C:\\Windows\\Fonts\\seguiemj.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/System/Library/Fonts/Supplemental/Geeza Pro.ttf",
    "/usr/share/fonts/opentype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansArabic-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_list_measures_intrinsic_then_fr_children() {
        let mut list = Component::list(
            Axis::Vertical,
            [
                (Sizing::Intrinsic, Component::text("Demo")),
                (
                    Sizing::fr(1.0),
                    Component::sized(SizeSource::Parent, SizeSource::Parent, None),
                ),
            ],
        );

        let size = list.layout(SizeRecommendation::exact(800.0, 600.0));

        assert_eq!(size, Size::new(800.0, 600.0));
    }

    #[test]
    fn scrollable_passes_finite_viewport_recommendation_to_child() {
        let mut root = Component::scrollable(
            Axis::Vertical,
            Component::list(
                Axis::Vertical,
                [(
                    Sizing::fr(1.0),
                    Component::fill(
                        Color::WHITE,
                        Component::sized(SizeSource::Parent, SizeSource::Parent, None),
                    ),
                )],
            ),
        );

        let size = root.layout(SizeRecommendation::exact(800.0, 600.0));

        assert_eq!(size, Size::new(800.0, 600.0));
        match root.kind {
            Kind::Scrollable(scrollable) => {
                assert_eq!(scrollable.child.rect().size(), Size::new(780.0, 600.0));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn horizontal_sized_can_copy_width_without_inflating_height() {
        let mut row = Component::list(
            Axis::Horizontal,
            [
                (
                    Sizing::Intrinsic,
                    Component::button(Component::text("Demo")),
                ),
                (
                    Sizing::fr(1.0),
                    Component::sized(SizeSource::Parent, SizeSource::Zero, None),
                ),
            ],
        );

        let size = row.layout(SizeRecommendation::exact(800.0, 600.0));

        assert_eq!(size.width, 800.0);
        assert!(size.height > 0.0);
        assert!(size.height < 600.0);
    }

    #[test]
    fn sized_passes_recommendation_to_child_and_selects_each_axis() {
        let mut component = Component::sized(
            SizeSource::Parent,
            SizeSource::Child,
            Some(Component::sized(SizeSource::Zero, SizeSource::Parent, None)),
        );

        let size = component.layout(SizeRecommendation::exact(320.0, 240.0));

        assert_eq!(size, Size::new(320.0, 240.0));
        match &component.kind {
            Kind::Sized(sized) => {
                assert_eq!(
                    sized.child.as_ref().unwrap().rect().size(),
                    Size::new(0.0, 240.0)
                );
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn text_runs_split_by_unicode_script() {
        assert_eq!(
            script_runs("Hello 世界 مرحبا")
                .into_iter()
                .map(|run| (run.value, run.script))
                .collect::<Vec<_>>(),
            vec![
                ("Hello ", Some(Script::Latin)),
                ("世界 ", Some(Script::Han)),
                ("مرحبا", Some(Script::Arabic)),
            ]
        );
    }

    #[test]
    fn script_runs_cover_scripts_from_unicode_data() {
        assert_eq!(
            script_runs("Rust𞤀")
                .into_iter()
                .map(|run| (run.value, run.script))
                .collect::<Vec<_>>(),
            vec![("Rust", Some(Script::Latin)), ("𞤀", Some(Script::Adlam)),]
        );
    }

    #[test]
    fn font_runs_switch_fonts_without_losing_script_information() {
        let run = TextRun {
            value: "Hello 世界 ",
            script: Some(Script::Latin),
        };

        assert_eq!(
            split_font_runs(run, |character| {
                if matches!(character, '世' | '界') {
                    Some(1)
                } else {
                    Some(0)
                }
            }),
            vec![
                FontRun {
                    value: "Hello ",
                    script: Some(Script::Latin),
                    font_index: 0,
                },
                FontRun {
                    value: "世界",
                    script: Some(Script::Latin),
                    font_index: 1,
                },
                FontRun {
                    value: " ",
                    script: Some(Script::Latin),
                    font_index: 0,
                },
            ]
        );
    }

    #[test]
    fn installed_fonts_cover_mixed_language_text_when_available() {
        let Some(engine) = TextEngine::new() else {
            return;
        };
        let characters = "Hello 世界 مرحبا"
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<Vec<_>>();
        if !characters
            .iter()
            .all(|character| engine.font_index_for(*character).is_some())
        {
            return;
        }

        for character in characters {
            let font = &engine.fonts[engine.font_index_for(character).unwrap()];
            assert_ne!(
                unsafe { ft::FT_Get_Char_Index(font.face, character as ft::FT_ULong) },
                0,
                "no fallback font covers {character:?}"
            );
        }
    }
}
