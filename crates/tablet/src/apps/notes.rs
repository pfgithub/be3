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

pub(crate) struct NotesApp {
    selected_tool: Tool,
    strokes: Vec<Stroke>,
    active_stroke: Option<usize>,
    active_eraser: bool,
}

impl NotesApp {
    pub(crate) fn new() -> Self {
        Self {
            selected_tool: Tool::Pen,
            strokes: Vec::new(),
            active_stroke: None,
            active_eraser: false,
        }
    }

    pub(crate) fn pointer_pressed(
        &mut self,
        size: Vector<2, f32>,
        position: Vector<2, f32>,
    ) -> bool {
        if let Some(tool) = tool_at_position(position) {
            self.selected_tool = tool;
            self.active_stroke = None;
            self.active_eraser = false;
            return true;
        }

        if !canvas_rect(size).contains(position) {
            self.active_stroke = None;
            self.active_eraser = false;
            return false;
        }

        match self.selected_tool {
            Tool::Pen | Tool::Highlighter => {
                self.strokes.push(Stroke {
                    tool: self.selected_tool,
                    points: vec![position],
                });
                self.active_stroke = Some(self.strokes.len() - 1);
                true
            }
            Tool::Eraser => {
                self.active_stroke = None;
                self.active_eraser = true;
                self.erase_at(position)
            }
        }
    }

    pub(crate) fn pointer_moved(&mut self, size: Vector<2, f32>, position: Vector<2, f32>) -> bool {
        if !canvas_rect(size).contains(position) {
            return false;
        }

        match self.selected_tool {
            Tool::Pen | Tool::Highlighter => {
                let Some(index) = self.active_stroke else {
                    return false;
                };
                self.strokes[index].points.push(position);
                true
            }
            Tool::Eraser if self.active_eraser => self.erase_at(position),
            Tool::Eraser => false,
        }
    }

    pub(crate) fn pointer_released(
        &mut self,
        _size: Vector<2, f32>,
        _position: Vector<2, f32>,
    ) -> bool {
        let was_drawing = self.active_stroke.take().is_some();
        let was_erasing = self.active_eraser;
        self.active_eraser = false;
        self.strokes.retain(|stroke| !stroke.points.is_empty());
        was_drawing || was_erasing
    }

    pub(crate) fn draw(&self, scene: &mut Scene, text: &mut TextEngine, size: Vector<2, f32>) {
        self.draw_toolbar(scene, text, size);

        let canvas = canvas_rect(size);
        scene.stroke_rect(canvas, 2.0, Color::BLACK);
        for stroke in &self.strokes {
            stroke.draw(scene);
        }
    }

    #[cfg(test)]
    pub(crate) fn stroke_count(&self) -> usize {
        self.strokes.len()
    }

    #[cfg(test)]
    pub(crate) fn stroke_points(&self, index: usize) -> Option<&[Vector<2, f32>]> {
        self.strokes
            .get(index)
            .map(|stroke| stroke.points.as_slice())
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

    fn erase_at(&mut self, position: Vector<2, f32>) -> bool {
        let previous_len = self.strokes.len();
        let mut changed = false;
        let mut replacement = Vec::new();

        for stroke in self.strokes.drain(..) {
            if stroke.erase_at(position, ERASER_WIDTH * 0.5) {
                changed = true;
                replacement.extend(stroke.split_away_from(position, ERASER_WIDTH * 0.5));
            } else {
                replacement.push(stroke);
            }
        }

        changed |= replacement.len() != previous_len;
        self.strokes = replacement;
        changed
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

struct Stroke {
    tool: Tool,
    points: Vec<Vector<2, f32>>,
}

impl Stroke {
    fn draw(&self, scene: &mut Scene) {
        let width = match self.tool {
            Tool::Pen => PEN_WIDTH,
            Tool::Highlighter => HIGHLIGHTER_WIDTH,
            Tool::Eraser => return,
        };
        let color = match self.tool {
            Tool::Pen => Color::BLACK,
            Tool::Highlighter => Color::GRAY,
            Tool::Eraser => return,
        };

        if let Some(point) = self.points.first().copied() {
            if self.points.len() == 1 {
                scene.push_line(point, point, width, color);
            }
        }

        for points in self.points.windows(2) {
            scene.push_line(points[0], points[1], width, color);
        }
    }

    fn erase_at(&self, position: Vector<2, f32>, radius: f32) -> bool {
        if self.points.len() <= 1 {
            return self
                .points
                .first()
                .is_some_and(|point| distance(*point, position) <= radius);
        }

        self.points
            .windows(2)
            .any(|points| segment_distance(points[0], points[1], position) <= radius)
    }

    fn split_away_from(self, position: Vector<2, f32>, radius: f32) -> Vec<Self> {
        if self.points.len() <= 1 {
            return Vec::new();
        }

        let mut output = Vec::new();
        let mut current = Vec::new();
        for points in self.points.windows(2) {
            let start = points[0];
            let end = points[1];
            if segment_distance(start, end, position) <= radius {
                if current.len() > 1 {
                    output.push(Self {
                        tool: self.tool,
                        points: std::mem::take(&mut current),
                    });
                }
                current.clear();
            } else {
                if current.is_empty() {
                    current.push(start);
                }
                current.push(end);
            }
        }

        if current.len() > 1 {
            output.push(Self {
                tool: self.tool,
                points: current,
            });
        }
        output
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

fn distance(a: Vector<2, f32>, b: Vector<2, f32>) -> f32 {
    let delta = a - b;
    (delta[0] * delta[0] + delta[1] * delta[1]).sqrt()
}

fn segment_distance(start: Vector<2, f32>, end: Vector<2, f32>, position: Vector<2, f32>) -> f32 {
    let segment = end - start;
    let length_squared = segment[0] * segment[0] + segment[1] * segment[1];
    if length_squared <= f32::EPSILON {
        return distance(start, position);
    }

    let offset = position - start;
    let projection =
        ((offset[0] * segment[0] + offset[1] * segment[1]) / length_squared).clamp(0.0, 1.0);
    distance(
        Vector::new(
            start[0] + segment[0] * projection,
            start[1] + segment[1] * projection,
        ),
        position,
    )
}

#[cfg(test)]
#[path = "notes/tests.rs"]
mod tests;
