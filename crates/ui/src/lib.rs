use font8x8::UnicodeFonts;
use minifb::{Key, Window, WindowOptions};

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

    fn cross(self, axis: Axis) -> Option<f32> {
        match axis {
            Axis::Horizontal => self.height,
            Axis::Vertical => self.width,
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
pub struct Color(u32);

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(((red as u32) << 16) | ((green as u32) << 8) | blue as u32)
    }
}

#[derive(Clone, Debug)]
pub struct Component {
    kind: Kind,
    rect: Rect,
}

#[derive(Clone, Debug)]
enum Kind {
    Void,
    Fill(Fill),
    Text(Text),
    Button(Box<Component>),
    List(List),
    Scrollable(Scrollable),
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
    pub fn void() -> Self {
        Self::new(Kind::Void)
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
            Kind::Void => Size::new(
                recommendation.width.unwrap_or(0.0),
                recommendation.height.unwrap_or(0.0),
            ),
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
            Kind::Void | Kind::Text(_) => {}
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

    fn paint(&self, canvas: &mut Canvas, offset_x: f32, offset_y: f32) {
        let x = offset_x + self.rect.x;
        let y = offset_y + self.rect.y;
        match &self.kind {
            Kind::Void => {}
            Kind::Fill(fill) => {
                canvas.fill_rect(
                    Rect::new(x, y, self.rect.width, self.rect.height),
                    fill.color,
                );
                fill.child.paint(canvas, x, y);
            }
            Kind::Text(text) => canvas.draw_text(x, y + 2.0, &text.value, Color::BLACK),
            Kind::Button(child) => {
                canvas.fill_rect(
                    Rect::new(x, y, self.rect.width, self.rect.height),
                    Color::rgb(0xf2, 0xf2, 0xf2),
                );
                canvas.stroke_rect(
                    Rect::new(x, y, self.rect.width, self.rect.height),
                    Color::BLACK,
                );
                child.paint(canvas, x, y);
            }
            Kind::List(list) => {
                for child in &list.children {
                    child.component.paint(canvas, x, y);
                }
            }
            Kind::Scrollable(scrollable) => {
                scrollable.child.paint(canvas, x, y);
                let bar_color = Color::rgb(0xc0, 0xc0, 0xc0);
                match scrollable.axis {
                    Axis::Vertical => canvas.fill_rect(
                        Rect::new(
                            x + self.rect.width - SCROLLBAR_SIZE,
                            y,
                            SCROLLBAR_SIZE,
                            self.rect.height,
                        ),
                        bar_color,
                    ),
                    Axis::Horizontal => canvas.fill_rect(
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

impl Text {
    fn layout(&self) -> Size {
        Size::new(self.value.chars().count() as f32 * 10.0, 20.0)
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

        Size::from_axes(
            axis,
            intrinsic_main + fr_main,
            recommendation
                .cross(axis)
                .unwrap_or(max_cross)
                .max(max_cross),
        )
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
    window: Window,
    canvas: Canvas,
}

impl UiWindow {
    pub fn new(title: &str, width: usize, height: usize) -> Result<Self, minifb::Error> {
        let window = Window::new(title, width, height, WindowOptions::default())?;
        Ok(Self {
            window,
            canvas: Canvas::new(width, height),
        })
    }

    pub fn run(
        &mut self,
        mut root: Component,
        initial_recommendation: SizeRecommendation,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let size = root.layout(initial_recommendation);
        root.place(Rect::new(0.0, 0.0, size.width, size.height));

        while self.window.is_open() && !self.window.is_key_down(Key::Escape) {
            self.canvas.clear(Color::rgb(0xee, 0xee, 0xee));
            root.paint(&mut self.canvas, 0.0, 0.0);
            self.window.update_with_buffer(
                &self.canvas.buffer,
                self.canvas.width,
                self.canvas.height,
            )?;
        }
        Ok(())
    }
}

struct Canvas {
    width: usize,
    height: usize,
    buffer: Vec<u32>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            buffer: vec![0; width * height],
        }
    }

    fn clear(&mut self, color: Color) {
        self.buffer.fill(color.0);
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let x0 = rect.x.max(0.0).floor() as usize;
        let y0 = rect.y.max(0.0).floor() as usize;
        let x1 = (rect.x + rect.width).min(self.width as f32).ceil() as usize;
        let y1 = (rect.y + rect.height).min(self.height as f32).ceil() as usize;

        for y in y0..y1 {
            let row = y * self.width;
            for x in x0..x1 {
                self.buffer[row + x] = color.0;
            }
        }
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
        let mut pen_x = x as i32;
        let pen_y = y as i32;
        for character in value.chars() {
            if let Some(glyph) = font8x8::BASIC_FONTS.get(character) {
                for (row, bits) in glyph.iter().enumerate() {
                    for col in 0..8 {
                        if bits & (1 << col) != 0 {
                            self.set_pixel(pen_x + col, pen_y + row as i32, color);
                        }
                    }
                }
            }
            pen_x += 10;
        }
    }

    fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 {
            return;
        }
        let x = x as usize;
        let y = y as usize;
        if x < self.width && y < self.height {
            self.buffer[y * self.width + x] = color.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_list_measures_intrinsic_then_fr_children() {
        let mut list = Component::list(
            Axis::Vertical,
            [
                (Sizing::Intrinsic, Component::text("Demo")),
                (Sizing::fr(1.0), Component::void()),
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
                    Component::fill(Color::WHITE, Component::void()),
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
}
