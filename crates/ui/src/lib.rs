use freetype::freetype as ft;
use harfbuzz_rs::{shape, Face as HbFace, Font as HbFont, Tag, UnicodeBuffer};
use minifb::{Key, Window, WindowOptions};
use once_cell::sync::OnceCell;
use std::ffi::CString;
use std::path::Path;
use std::ptr;
use unicode_script::{Script, UnicodeScript};

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

    fn paint(&self, canvas: &mut Canvas, offset_x: f32, offset_y: f32) {
        let x = offset_x + self.rect.x;
        let y = offset_y + self.rect.y;
        match &self.kind {
            Kind::Sized(sized) => {
                if let Some(child) = &sized.child {
                    child.paint(canvas, x, y);
                }
            }
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
        if let Some(mut engine) = TextEngine::new() {
            engine.draw(self, x, y, value, color);
        }
    }

    fn blend_pixel(&mut self, x: i32, y: i32, color: Color, alpha: u8) {
        if x < 0 || y < 0 || alpha == 0 {
            return;
        }
        let x = x as usize;
        let y = y as usize;
        if x >= self.width || y >= self.height {
            return;
        }

        let index = y * self.width + x;
        self.buffer[index] = blend_color(self.buffer[index], color.0, alpha);
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

    fn draw(&mut self, canvas: &mut Canvas, x: f32, y: f32, value: &str, color: Color) {
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
                        paint_glyph_bitmap(canvas, bitmap_x, bitmap_y, &(*slot).bitmap, color);
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

fn paint_glyph_bitmap(canvas: &mut Canvas, x: f32, y: f32, bitmap: &ft::FT_Bitmap, color: Color) {
    let width = bitmap.width as i32;
    let rows = bitmap.rows as i32;
    let pitch = bitmap.pitch.unsigned_abs() as usize;
    let byte_len = pitch * rows.max(0) as usize;
    if bitmap.buffer.is_null() || width <= 0 || rows <= 0 || byte_len == 0 {
        return;
    }
    let buffer = unsafe { std::slice::from_raw_parts(bitmap.buffer, byte_len) };
    let origin_x = x.round() as i32;
    let origin_y = y.round() as i32;

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
                canvas.blend_pixel(origin_x + col, origin_y + row, color, alpha);
            }
        }
    }
}

fn blend_color(background: u32, foreground: u32, alpha: u8) -> u32 {
    let alpha = alpha as u32;
    let inverse = 255 - alpha;
    let red = (((foreground >> 16) & 0xff) * alpha + ((background >> 16) & 0xff) * inverse) / 255;
    let green = (((foreground >> 8) & 0xff) * alpha + ((background >> 8) & 0xff) * inverse) / 255;
    let blue = ((foreground & 0xff) * alpha + (background & 0xff) * inverse) / 255;
    (red << 16) | (green << 8) | blue
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
