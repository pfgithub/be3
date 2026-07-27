use crate::renderer::{Color, Rect, Scene, Vector};
use crate::text::TextEngine;

const STATUS_BAR_HEIGHT: f32 = 54.0;
const OUTER_MARGIN: f32 = 22.0;
const TOOLBAR_HEIGHT: f32 = 64.0;
const TOOL_BUTTON_WIDTH: f32 = 142.0;
const TOOL_BUTTON_GAP: f32 = 14.0;
const PEN_WIDTH: f32 = 3.0;
const HIGHLIGHTER_WIDTH: f32 = 18.0;
const ERASER_WIDTH: f32 = 28.0;
const PEN_COVERAGE: u8 = 255;
const HIGHLIGHTER_COVERAGE: u8 = 128;

pub(crate) struct NotesApp {
    selected_tool: Tool,
    bitmap: Option<Bitmap>,
    active_position: Option<Vector<2, f32>>,
}

impl NotesApp {
    pub(crate) fn new() -> Self {
        Self {
            selected_tool: Tool::Pen,
            bitmap: None,
            active_position: None,
        }
    }

    pub(crate) fn pointer_pressed(
        &mut self,
        size: Vector<2, f32>,
        position: Vector<2, f32>,
    ) -> bool {
        if let Some(tool) = tool_at_position(position) {
            self.selected_tool = tool;
            self.active_position = None;
            return true;
        }

        let canvas = canvas_rect(size);
        if !canvas.contains(position) {
            self.active_position = None;
            return false;
        }

        self.ensure_bitmap(canvas);
        let bitmap_position = self.bitmap_position(canvas, position);
        self.active_position = Some(bitmap_position);
        self.draw_segment(bitmap_position, bitmap_position);
        true
    }

    pub(crate) fn pointer_moved(&mut self, size: Vector<2, f32>, position: Vector<2, f32>) -> bool {
        let canvas = canvas_rect(size);
        let Some(previous_position) = self.active_position else {
            return false;
        };

        let bitmap_position = self.bitmap_position(canvas, position);
        self.draw_segment(previous_position, bitmap_position);
        self.active_position = Some(bitmap_position);
        true
    }

    pub(crate) fn pointer_released(
        &mut self,
        _size: Vector<2, f32>,
        _position: Vector<2, f32>,
    ) -> bool {
        self.active_position.take().is_some()
    }

    pub(crate) fn draw(&self, scene: &mut Scene, text: &mut TextEngine, size: Vector<2, f32>) {
        self.draw_toolbar(scene, text, size);

        let canvas = canvas_rect(size);
        scene.stroke_rect(canvas, 2.0, Color::BLACK);
        let Some(bitmap) = &self.bitmap else {
            return;
        };
        if let Some((uv_min, uv_max)) = scene.add_bitmap(bitmap.size(), &bitmap.pixels) {
            scene.push_quad(canvas, uv_min, uv_max, Color::BLACK);
        }
    }

    fn draw_toolbar(&self, scene: &mut Scene, text: &mut TextEngine, size: Vector<2, f32>) {
        scene.push_rect(
            Rect::new(
                Vector::new(0.0, STATUS_BAR_HEIGHT + TOOLBAR_HEIGHT - 1.0),
                Vector::new(size[0], 1.0),
            ),
            Color::BLACK,
        );

        for tool in Tool::ALL {
            let rect = tool_button_rect(tool);
            let is_selected = tool == self.selected_tool;
            if is_selected {
                scene.push_rect(rect, Color::BLACK);
            } else {
                scene.stroke_rect(rect, 2.0, Color::BLACK);
            }
            text.draw(
                scene,
                Vector::new(rect.position[0] + 18.0, rect.position[1] + 16.0),
                tool.label(),
                if is_selected {
                    Color::WHITE
                } else {
                    Color::BLACK
                },
            );
        }
    }

    fn ensure_bitmap(&mut self, canvas: Rect) {
        if self.bitmap.is_some() {
            return;
        }
        self.bitmap = Some(Bitmap::new(
            canvas.size[0].ceil() as u32,
            canvas.size[1].ceil() as u32,
        ));
    }

    fn bitmap_position(&self, canvas: Rect, position: Vector<2, f32>) -> Vector<2, f32> {
        let bitmap = self.bitmap.as_ref().expect("bitmap should be initialized");
        Vector::new(
            (position[0] - canvas.position[0]) * bitmap.width as f32 / canvas.size[0],
            (position[1] - canvas.position[1]) * bitmap.height as f32 / canvas.size[1],
        )
    }

    fn draw_segment(&mut self, start: Vector<2, f32>, end: Vector<2, f32>) {
        let (width, coverage) = match self.selected_tool {
            Tool::Pen => (PEN_WIDTH, Some(PEN_COVERAGE)),
            Tool::Highlighter => (HIGHLIGHTER_WIDTH, Some(HIGHLIGHTER_COVERAGE)),
            Tool::Eraser => (ERASER_WIDTH, None),
        };
        self.bitmap
            .as_mut()
            .expect("bitmap should be initialized")
            .draw_line(start, end, width, coverage);
    }

    #[cfg(test)]
    pub(crate) fn coverage_at(&self, size: Vector<2, f32>, position: Vector<2, f32>) -> u8 {
        let canvas = canvas_rect(size);
        let Some(bitmap) = &self.bitmap else {
            return 0;
        };
        let position = self.bitmap_position(canvas, position);
        bitmap.coverage_at(position[0].floor() as i32, position[1].floor() as i32)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tool {
    Pen,
    Highlighter,
    Eraser,
}

impl Tool {
    const ALL: [Self; 3] = [Self::Pen, Self::Highlighter, Self::Eraser];

    fn label(self) -> &'static str {
        match self {
            Self::Pen => "Pen",
            Self::Highlighter => "Highlighter",
            Self::Eraser => "Eraser",
        }
    }
}

struct Bitmap {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Bitmap {
    fn new(width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        Self {
            width,
            height,
            pixels: vec![0; (width * height) as usize],
        }
    }

    fn size(&self) -> Vector<2, u32> {
        Vector::new(self.width, self.height)
    }

    fn draw_line(
        &mut self,
        start: Vector<2, f32>,
        end: Vector<2, f32>,
        width: f32,
        coverage: Option<u8>,
    ) {
        let delta = end - start;
        let distance = (delta[0] * delta[0] + delta[1] * delta[1]).sqrt();
        let steps = distance.ceil().max(1.0) as u32;
        for step in 0..=steps {
            let progress = step as f32 / steps as f32;
            let center = Vector::new(
                start[0] + delta[0] * progress,
                start[1] + delta[1] * progress,
            );
            self.draw_circle(center, width * 0.5, coverage);
        }
    }

    fn draw_circle(&mut self, center: Vector<2, f32>, radius: f32, coverage: Option<u8>) {
        let min_x = (center[0] - radius).floor() as i32;
        let max_x = (center[0] + radius).ceil() as i32;
        let min_y = (center[1] - radius).floor() as i32;
        let max_y = (center[1] + radius).ceil() as i32;
        let radius_squared = radius * radius;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let offset_x = x as f32 + 0.5 - center[0];
                let offset_y = y as f32 + 0.5 - center[1];
                if offset_x * offset_x + offset_y * offset_y > radius_squared {
                    continue;
                }
                let Some(index) = self.pixel_index(x, y) else {
                    continue;
                };
                match coverage {
                    Some(coverage) => self.pixels[index] = self.pixels[index].max(coverage),
                    None => self.pixels[index] = 0,
                }
            }
        }
    }

    fn pixel_index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    #[cfg(test)]
    fn coverage_at(&self, x: i32, y: i32) -> u8 {
        self.pixel_index(x, y).map_or(0, |index| self.pixels[index])
    }
}

fn tool_at_position(position: Vector<2, f32>) -> Option<Tool> {
    Tool::ALL
        .into_iter()
        .find(|tool| tool_button_rect(*tool).contains(position))
}

fn tool_button_rect(tool: Tool) -> Rect {
    let index = match tool {
        Tool::Pen => 0.0,
        Tool::Highlighter => 1.0,
        Tool::Eraser => 2.0,
    };
    Rect::new(
        Vector::new(
            OUTER_MARGIN + index * (TOOL_BUTTON_WIDTH + TOOL_BUTTON_GAP),
            STATUS_BAR_HEIGHT + 12.0,
        ),
        Vector::new(TOOL_BUTTON_WIDTH, 40.0),
    )
}

fn canvas_rect(size: Vector<2, f32>) -> Rect {
    let top = STATUS_BAR_HEIGHT + TOOLBAR_HEIGHT + OUTER_MARGIN;
    Rect::new(
        Vector::new(OUTER_MARGIN, top),
        Vector::new(
            (size[0] - OUTER_MARGIN * 2.0).max(1.0),
            (size[1] - top - OUTER_MARGIN).max(1.0),
        ),
    )
}

#[cfg(test)]
#[path = "notes/tests.rs"]
mod tests;
