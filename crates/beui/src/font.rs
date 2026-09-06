use std::collections::HashMap;
use std::ffi::CString;
use std::ops::Range;
use std::path::Path;
use std::ptr;
use std::rc::Rc;

use freetype::freetype as ft;
use harfbuzz_rs::{shape, Face as HbFace, Font as HbFont, Owned, Tag, UnicodeBuffer};
use unicode_script::{Script, UnicodeScript};

use crate::geometry::{pos2, vec2, Pos2, Rect, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FontFamily {
    Proportional,
    Monospace,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FontId {
    pub size: f32,
    pub family: FontFamily,
}

impl FontId {
    pub fn proportional(size: f32) -> Self {
        Self {
            size,
            family: FontFamily::Proportional,
        }
    }

    pub fn monospace(size: f32) -> Self {
        Self {
            size,
            family: FontFamily::Monospace,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GlyphId {
    face: usize,
    glyph: u32,
    pixel_size: u32,
}

pub struct GlyphImage {
    pub width: u32,
    pub height: u32,
    left: i32,
    top: i32,
    pub pixels: Vec<u8>,
}

#[derive(Clone)]
pub struct Glyph {
    pub id: GlyphId,
    pub image: Rc<GlyphImage>,
    pub offset: Vec2,
}

#[derive(Clone)]
pub struct Galley {
    inner: Rc<GalleyData>,
}

struct GalleyData {
    size: Vec2,
    line_height: f32,
    glyphs: Vec<Glyph>,
    lines: Vec<GalleyLine>,
}

struct GalleyLine {
    top: f32,
    range: Range<usize>,
    cursors: Vec<(usize, f32)>,
}

impl GalleyLine {
    fn x(&self, index: usize) -> f32 {
        let mut x = 0.0;
        for (at, candidate) in &self.cursors {
            if *at > index {
                break;
            }
            x = *candidate;
        }
        x
    }
}

impl Galley {
    pub fn size(&self) -> Vec2 {
        self.inner.size
    }

    pub fn line_height(&self) -> f32 {
        self.inner.line_height
    }

    pub fn cursor_pos(&self, origin: Pos2, index: usize) -> Pos2 {
        match self.line_of(index) {
            Some(line) => origin + vec2(line.x(index), line.top),
            None => origin,
        }
    }

    pub fn cursor_at(&self, origin: Pos2, pos: Pos2) -> usize {
        let Some(line) = self.line_at(pos.y - origin.y) else {
            return 0;
        };
        let target = pos.x - origin.x;
        let mut closest = line.range.start;
        let mut distance = f32::INFINITY;
        for (index, x) in &line.cursors {
            let candidate = (*x - target).abs();
            if candidate < distance {
                distance = candidate;
                closest = *index;
            }
        }
        closest
    }

    pub fn selection_rects(&self, origin: Pos2, range: Range<usize>) -> Vec<Rect> {
        self.inner
            .lines
            .iter()
            .filter_map(|line| {
                let start = range.start.max(line.range.start);
                let end = range.end.min(line.range.end);
                if start >= end {
                    return None;
                }
                let top = origin.y + line.top;
                Some(Rect::from_min_max(
                    pos2(origin.x + line.x(start), top),
                    pos2(origin.x + line.x(end), top + self.inner.line_height),
                ))
            })
            .collect()
    }

    pub fn glyphs(&self) -> &[Glyph] {
        &self.inner.glyphs
    }

    fn line_of(&self, index: usize) -> Option<&GalleyLine> {
        self.inner
            .lines
            .iter()
            .find(|line| index <= line.range.end)
            .or_else(|| self.inner.lines.last())
    }

    fn line_at(&self, y: f32) -> Option<&GalleyLine> {
        self.inner
            .lines
            .iter()
            .find(|line| y < line.top + self.inner.line_height)
            .or_else(|| self.inner.lines.last())
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct GalleyKey {
    text: String,
    size: u32,
    family: FontFamily,
    wrap: u32,
}

const GALLEY_CACHE_LIMIT: usize = 4096;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontSource {
    File(&'static str),
    Memory(&'static [u8]),
}

#[derive(Clone, Debug)]
pub struct FontSources {
    pub proportional: Vec<FontSource>,
    pub monospace: Vec<FontSource>,
    pub fallback: Vec<FontSource>,
}

impl FontSources {
    pub fn installed() -> Self {
        Self {
            proportional: files(PROPORTIONAL_CANDIDATES),
            monospace: files(MONOSPACE_CANDIDATES),
            fallback: files(FALLBACK_CANDIDATES),
        }
    }
}

impl Default for FontSources {
    fn default() -> Self {
        Self::installed()
    }
}

fn files(candidates: &[&'static str]) -> Vec<FontSource> {
    candidates.iter().copied().map(FontSource::File).collect()
}

fn available(source: &FontSource) -> bool {
    match source {
        FontSource::File(path) => Path::new(path).exists(),
        FontSource::Memory(_) => true,
    }
}

struct FaceData {
    source: FontSource,
    face: ft::FT_Face,
    font: Owned<HbFont<'static>>,
}

#[derive(Clone, Copy)]
struct ShapedGlyph {
    face: usize,
    glyph: u32,
    cluster: usize,
    x_advance: f32,
    x_offset: f32,
    y_offset: f32,
}

pub(crate) struct Fonts {
    library: ft::FT_Library,
    faces: Vec<FaceData>,
    proportional: Vec<usize>,
    monospace: Vec<usize>,
    glyphs: HashMap<GlyphId, Rc<GlyphImage>>,
    galleys: HashMap<GalleyKey, Galley>,
    pixels_per_point: f32,
}

impl Fonts {
    pub(crate) fn new(sources: &FontSources) -> Self {
        let mut library = ptr::null_mut();
        let opened = unsafe { ft::FT_Init_FreeType(&mut library) == 0 };
        let mut fonts = Self {
            library: if opened { library } else { ptr::null_mut() },
            faces: Vec::new(),
            proportional: Vec::new(),
            monospace: Vec::new(),
            glyphs: HashMap::new(),
            galleys: HashMap::new(),
            pixels_per_point: 1.0,
        };
        if opened {
            fonts.load_families(sources);
        }
        fonts
    }

    pub(crate) fn pixels_per_point(&self) -> f32 {
        self.pixels_per_point
    }

    pub(crate) fn set_pixels_per_point(&mut self, pixels_per_point: f32) {
        if pixels_per_point == self.pixels_per_point {
            return;
        }
        self.pixels_per_point = pixels_per_point;
        self.galleys.clear();
        self.glyphs.clear();
    }

    fn load_families(&mut self, sources: &FontSources) {
        let proportional = self.load_chain(&sources.proportional);
        let monospace = self.load_chain(&sources.monospace);
        let fallback = self.load_chain(&sources.fallback);
        self.proportional = chain(&proportional, &[&monospace, &fallback]);
        self.monospace = chain(&monospace, &[&proportional, &fallback]);
    }

    fn load_chain(&mut self, sources: &[FontSource]) -> Vec<usize> {
        sources
            .iter()
            .copied()
            .filter(available)
            .filter_map(|source| self.load_face(source))
            .collect()
    }

    fn load_face(&mut self, source: FontSource) -> Option<usize> {
        if let Some(index) = self.faces.iter().position(|face| face.source == source) {
            return Some(index);
        }
        let mut face = ptr::null_mut();
        let hb_face = match source {
            FontSource::File(path) => {
                let name = CString::new(path).ok()?;
                unsafe {
                    if ft::FT_New_Face(self.library, name.as_ptr(), 0, &mut face) != 0 {
                        return None;
                    }
                }
                HbFace::from_file(path, 0).ok()
            }
            FontSource::Memory(bytes) => {
                unsafe {
                    if ft::FT_New_Memory_Face(
                        self.library,
                        bytes.as_ptr(),
                        bytes.len() as ft::FT_Long,
                        0,
                        &mut face,
                    ) != 0
                    {
                        return None;
                    }
                }
                Some(HbFace::from_bytes(bytes, 0))
            }
        };
        let Some(hb_face) = hb_face else {
            unsafe { ft::FT_Done_Face(face) };
            return None;
        };
        self.faces.push(FaceData {
            source,
            face,
            font: HbFont::new(hb_face),
        });
        Some(self.faces.len() - 1)
    }

    fn family(&self, family: FontFamily) -> &[usize] {
        match family {
            FontFamily::Proportional => &self.proportional,
            FontFamily::Monospace => &self.monospace,
        }
    }

    pub(crate) fn layout(&mut self, text: &str, font: FontId, wrap_width: f32) -> Galley {
        let pixel_size = ((font.size * self.pixels_per_point).round() as u32).max(1);
        let wrap = (wrap_width * self.pixels_per_point).max(0.0);
        let key = GalleyKey {
            text: text.to_owned(),
            size: pixel_size,
            family: font.family,
            wrap: wrap.to_bits(),
        };
        if let Some(galley) = self.galleys.get(&key) {
            return galley.clone();
        }
        let galley = self.build(text, font.family, pixel_size, wrap);
        if self.galleys.len() >= GALLEY_CACHE_LIMIT {
            self.galleys.clear();
        }
        self.galleys.insert(key, galley.clone());
        galley
    }

    fn build(&mut self, text: &str, family: FontFamily, pixel_size: u32, wrap: f32) -> Galley {
        let (ascent, line_height) = self.metrics(family, pixel_size);
        let scale = self.pixels_per_point;
        let mut glyphs = Vec::new();
        let mut lines = Vec::new();
        let mut width = 0.0f32;
        let mut cursor = 0.0;
        let mut start = 0;

        for line in text.split('\n') {
            let shaped = self.shape_line(line, family, pixel_size);
            let runs = break_lines(&shaped, line, wrap);
            let last = runs.len() - 1;
            for (index, run) in runs.into_iter().enumerate() {
                let end = start
                    + match shaped.get(run.end) {
                        Some(glyph) if index < last => glyph.cluster,
                        _ => line.len(),
                    };
                let mut pen = 0.0;
                let mut cursors = Vec::new();
                for glyph in &shaped[run] {
                    let at = start + glyph.cluster;
                    if cursors.last().is_none_or(|(previous, _)| *previous != at) {
                        cursors.push((at, pen / scale));
                    }
                    let placed = self.place(*glyph, pixel_size, pen, cursor + ascent);
                    if let Some(placed) = placed {
                        glyphs.push(placed);
                    }
                    pen += glyph.x_advance;
                }
                cursors.push((end, pen / scale));
                width = width.max(pen);
                lines.push(GalleyLine {
                    top: cursor / scale,
                    range: cursors[0].0..end,
                    cursors,
                });
                cursor += line_height;
            }
            start += line.len() + 1;
        }

        Galley {
            inner: Rc::new(GalleyData {
                size: vec2(width.ceil() / scale, cursor.ceil() / scale),
                line_height: line_height / scale,
                glyphs,
                lines,
            }),
        }
    }

    fn place(
        &mut self,
        glyph: ShapedGlyph,
        pixel_size: u32,
        pen: f32,
        baseline: f32,
    ) -> Option<Glyph> {
        let id = GlyphId {
            face: glyph.face,
            glyph: glyph.glyph,
            pixel_size,
        };
        let image = self.image(id)?;
        if image.width == 0 || image.height == 0 {
            return None;
        }
        let x = (pen + glyph.x_offset).round() + image.left as f32;
        let y = (baseline - glyph.y_offset).round() - image.top as f32;
        Some(Glyph {
            id,
            image,
            offset: vec2(x, y),
        })
    }

    fn image(&mut self, key: GlyphId) -> Option<Rc<GlyphImage>> {
        if let Some(image) = self.glyphs.get(&key) {
            return Some(image.clone());
        }
        let image = Rc::new(self.rasterize(key)?);
        self.glyphs.insert(key, image.clone());
        Some(image)
    }

    fn rasterize(&self, key: GlyphId) -> Option<GlyphImage> {
        let face = self.faces.get(key.face)?.face;
        unsafe {
            if ft::FT_Set_Pixel_Sizes(face, 0, key.pixel_size) != 0 {
                return None;
            }
            if ft::FT_Load_Glyph(face, key.glyph, ft::FT_LOAD_DEFAULT as i32) != 0 {
                return None;
            }
            let slot = (*face).glyph;
            if ft::FT_Render_Glyph(slot, ft::FT_Render_Mode::FT_RENDER_MODE_NORMAL) != 0 {
                return None;
            }
            let bitmap = &(*slot).bitmap;
            Some(GlyphImage {
                width: bitmap.width,
                height: bitmap.rows,
                left: (*slot).bitmap_left,
                top: (*slot).bitmap_top,
                pixels: pixels(bitmap),
            })
        }
    }

    fn metrics(&self, family: FontFamily, pixel_size: u32) -> (f32, f32) {
        let fallback = (pixel_size as f32 * 0.8, pixel_size as f32 * 1.2);
        let Some(&index) = self.family(family).first() else {
            return fallback;
        };
        let face = self.faces[index].face;
        unsafe {
            if ft::FT_Set_Pixel_Sizes(face, 0, pixel_size) != 0 {
                return fallback;
            }
            let size = (*face).size;
            if size.is_null() {
                return fallback;
            }
            let metrics = (*size).metrics;
            (metrics.ascender as f32 / 64.0, metrics.height as f32 / 64.0)
        }
    }

    fn shape_line(&mut self, text: &str, family: FontFamily, pixel_size: u32) -> Vec<ShapedGlyph> {
        let mut glyphs = Vec::new();
        for run in self.font_runs(text, family) {
            let offset = run.start;
            let face = &mut self.faces[run.face];
            let scale = pixel_size as i32 * 64;
            face.font.set_scale(scale, scale);
            face.font.set_ppem(pixel_size, pixel_size);
            let mut buffer = UnicodeBuffer::new().add_str(&text[run.start..run.end]);
            if let Some(script) = run.script {
                let tag = script.as_iso15924_tag().to_be_bytes();
                buffer = buffer.set_script(Tag::new(
                    tag[0] as char,
                    tag[1] as char,
                    tag[2] as char,
                    tag[3] as char,
                ));
            }
            let output = shape(&face.font, buffer.guess_segment_properties(), &[]);
            glyphs.extend(
                output
                    .get_glyph_infos()
                    .iter()
                    .zip(output.get_glyph_positions())
                    .map(|(info, position)| ShapedGlyph {
                        face: run.face,
                        glyph: info.codepoint,
                        cluster: offset + info.cluster as usize,
                        x_advance: position.x_advance as f32 / 64.0,
                        x_offset: position.x_offset as f32 / 64.0,
                        y_offset: position.y_offset as f32 / 64.0,
                    }),
            );
        }
        glyphs
    }

    fn font_runs(&self, text: &str, family: FontFamily) -> Vec<FontRun> {
        let mut runs = Vec::new();
        for run in script_runs(text) {
            let mut start = run.start;
            let mut current = None;
            for (index, character) in text[run.start..run.end].char_indices() {
                let index = run.start + index;
                let face = self.face_for(character, family).or(current).unwrap_or(0);
                match current {
                    None => current = Some(face),
                    Some(previous) if previous == face => {}
                    Some(previous) => {
                        runs.push(FontRun {
                            start,
                            end: index,
                            script: run.script,
                            face: previous,
                        });
                        start = index;
                        current = Some(face);
                    }
                }
            }
            if let Some(face) = current {
                runs.push(FontRun {
                    start,
                    end: run.end,
                    script: run.script,
                    face,
                });
            }
        }
        runs
    }

    fn face_for(&self, character: char, family: FontFamily) -> Option<usize> {
        self.family(family).iter().copied().find(|index| unsafe {
            ft::FT_Get_Char_Index(self.faces[*index].face, character as ft::FT_ULong) != 0
        })
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe {
            for face in &self.faces {
                ft::FT_Done_Face(face.face);
            }
            if !self.library.is_null() {
                ft::FT_Done_FreeType(self.library);
            }
        }
    }
}

struct FontRun {
    start: usize,
    end: usize,
    script: Option<Script>,
    face: usize,
}

struct ScriptRun {
    start: usize,
    end: usize,
    script: Option<Script>,
}

fn chain(primary: &[usize], others: &[&[usize]]) -> Vec<usize> {
    let mut chain = primary.to_vec();
    for other in others {
        for index in *other {
            if !chain.contains(index) {
                chain.push(*index);
            }
        }
    }
    chain
}

fn script_runs(text: &str) -> Vec<ScriptRun> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut current = None;

    for (index, character) in text.char_indices() {
        let script = character.script();
        if matches!(script, Script::Common | Script::Inherited | Script::Unknown) {
            continue;
        }
        match current {
            None => current = Some(script),
            Some(previous) if previous == script => {}
            Some(_) => {
                runs.push(ScriptRun {
                    start,
                    end: index,
                    script: current,
                });
                start = index;
                current = Some(script);
            }
        }
    }

    if !text.is_empty() {
        runs.push(ScriptRun {
            start,
            end: text.len(),
            script: current,
        });
    }
    runs
}

fn break_lines(glyphs: &[ShapedGlyph], text: &str, wrap: f32) -> Vec<std::ops::Range<usize>> {
    let mut lines = Vec::new();
    if glyphs.is_empty() {
        lines.push(0..0);
        return lines;
    }
    if !wrap.is_finite() {
        lines.push(0..glyphs.len());
        return lines;
    }

    let mut start = 0;
    let mut width = 0.0;
    let mut candidate = None;

    for (index, glyph) in glyphs.iter().enumerate() {
        if index > start && width + glyph.x_advance > wrap {
            let end = candidate.filter(|end| *end > start).unwrap_or(index);
            lines.push(start..end);
            start = end;
            width = glyphs[start..index].iter().map(|it| it.x_advance).sum();
            candidate = None;
        }
        if index > start && follows_whitespace(text, glyph.cluster) {
            candidate = Some(index);
        }
        width += glyph.x_advance;
    }

    lines.push(start..glyphs.len());
    lines
}

fn follows_whitespace(text: &str, cluster: usize) -> bool {
    text.get(..cluster)
        .and_then(|before| before.chars().next_back())
        .is_some_and(char::is_whitespace)
}

fn pixels(bitmap: &ft::FT_Bitmap) -> Vec<u8> {
    let width = bitmap.width as usize;
    let rows = bitmap.rows as usize;
    let pitch = bitmap.pitch.unsigned_abs() as usize;
    if bitmap.buffer.is_null() || width == 0 || rows == 0 {
        return Vec::new();
    }
    let buffer = unsafe { std::slice::from_raw_parts(bitmap.buffer, pitch * rows) };
    let mut pixels = vec![0; width * rows];
    for row in 0..rows {
        let source = if bitmap.pitch >= 0 {
            row
        } else {
            rows - 1 - row
        };
        for column in 0..width {
            if let Some(value) = buffer.get(source * pitch + column) {
                pixels[row * width + column] = *value;
            }
        }
    }
    pixels
}

const PROPORTIONAL_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/opentype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "C:\\Windows\\Fonts\\segoeui.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
];

const MONOSPACE_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/opentype/noto/NotoSansMono-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
    "/System/Library/Fonts/SFNSMono.ttf",
    "/System/Library/Fonts/Menlo.ttc",
    "C:\\Windows\\Fonts\\consola.ttf",
    "C:\\Windows\\Fonts\\cour.ttf",
];

const FALLBACK_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansArabic-Regular.ttf",
    "/usr/share/fonts/truetype/unifont/unifont.ttf",
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "C:\\Windows\\Fonts\\msyh.ttc",
    "C:\\Windows\\Fonts\\seguisym.ttf",
];
