use std::{
    collections::{BTreeMap, HashMap},
    ffi::CString,
    path::{Path, PathBuf},
    ptr,
};

use eframe::egui::{self, Color32, Pos2, Rect, TextureHandle, Vec2};
use freetype::freetype as ft;
use harfbuzz_rs::{shape, Direction, Face as HbFace, Font as HbFont, Tag, UnicodeBuffer};
use unicode_script::{Script, UnicodeScript};

pub(super) const PIXEL_SIZE: u32 = 18;
pub(super) const LINE_HEIGHT: f32 = 25.0;

pub(super) struct TextRenderer {
    library: ft::FT_Library,
    fonts: Vec<FontFace>,
    glyphs: HashMap<GlyphKey, RasterizedGlyph>,
}

pub(super) struct DocumentLayout {
    pub size: Vec2,
    pub lines: Vec<LineLayout>,
    pub positions: Vec<Option<BytePosition>>,
}

pub(super) struct LineLayout {
    pub start: usize,
    pub end: usize,
    pub y: f32,
    pub width: f32,
    glyphs: Vec<PositionedGlyph>,
}

#[derive(Clone, Copy)]
pub(super) struct BytePosition {
    pub line: usize,
    pub x: f32,
}

struct FontFace {
    face: ft::FT_Face,
    path: PathBuf,
}

#[derive(Clone, Copy)]
struct PositionedGlyph {
    font_index: usize,
    id: u32,
    doc_byte: usize,
    x: f32,
    x_offset: f32,
    y_offset: f32,
    invisible: bool,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct GlyphKey {
    font_index: usize,
    id: u32,
}

struct RasterizedGlyph {
    texture: Option<TextureHandle>,
    size: Vec2,
    bearing: Vec2,
}

#[derive(Clone, Copy)]
struct TextRun<'a> {
    value: &'a str,
    start: usize,
    script: Option<Script>,
}

#[derive(Clone, Copy)]
struct FontRun<'a> {
    value: &'a str,
    start: usize,
    script: Option<Script>,
    font_index: usize,
}

struct DisplayLine {
    text: String,
    display_to_document: Vec<usize>,
}

impl TextRenderer {
    pub fn new() -> Result<Self, String> {
        let mut library = ptr::null_mut();
        if unsafe { ft::FT_Init_FreeType(&mut library) } != 0 {
            return Err("FreeType could not be initialized".to_owned());
        }

        let mut fonts = Vec::new();
        for path in font_paths() {
            let Some(path_text) = path.to_str() else {
                continue;
            };
            let Ok(path_c) = CString::new(path_text) else {
                continue;
            };
            let mut face = ptr::null_mut();
            if unsafe { ft::FT_New_Face(library, path_c.as_ptr(), 0, &mut face) } != 0 {
                continue;
            }
            if unsafe { ft::FT_Set_Pixel_Sizes(face, 0, PIXEL_SIZE) } != 0 {
                unsafe { ft::FT_Done_Face(face) };
                continue;
            }
            fonts.push(FontFace {
                face,
                path: path.clone(),
            });
        }

        if fonts.is_empty() {
            unsafe { ft::FT_Done_FreeType(library) };
            return Err("Verdana or a compatible fallback font was not found".to_owned());
        }

        Ok(Self {
            library,
            fonts,
            glyphs: HashMap::new(),
        })
    }

    pub fn layout(&self, bytes: &[u8]) -> DocumentLayout {
        let mut lines = Vec::new();
        let mut positions = vec![None; bytes.len() + 1];
        let mut start = 0;
        let mut line_index = 0;

        loop {
            let newline = bytes[start..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map(|offset| start + offset);
            let end = newline.unwrap_or(bytes.len());
            let display = display_line(&bytes[start..end], start, newline.is_some());
            let (glyphs, width, line_positions) = self.layout_line(bytes, &display);
            for (doc_byte, x) in line_positions {
                if let Some(position) = positions.get_mut(doc_byte) {
                    *position = Some(BytePosition {
                        line: line_index,
                        x,
                    });
                }
            }
            positions[start].get_or_insert(BytePosition {
                line: line_index,
                x: 0.0,
            });
            positions[end].get_or_insert(BytePosition {
                line: line_index,
                x: width,
            });
            lines.push(LineLayout {
                start,
                end,
                y: line_index as f32 * LINE_HEIGHT,
                width,
                glyphs,
            });

            let Some(newline) = newline else {
                break;
            };
            start = newline + 1;
            line_index += 1;
        }

        let width = lines.iter().map(|line| line.width).fold(0.0_f32, f32::max);
        DocumentLayout {
            size: Vec2::new(width + 24.0, lines.len() as f32 * LINE_HEIGHT + 16.0),
            lines,
            positions,
        }
    }

    fn layout_line(
        &self,
        document: &[u8],
        display: &DisplayLine,
    ) -> (Vec<PositionedGlyph>, f32, Vec<(usize, f32)>) {
        let mut glyphs = Vec::new();
        let mut positions = Vec::new();
        let mut pen_x: f32 = 0.0;

        for run in self.font_runs(&display.text) {
            let Ok(face) = HbFace::from_file(&self.fonts[run.font_index].path, 0) else {
                continue;
            };
            let mut font = HbFont::new(face);
            let scale = PIXEL_SIZE as i32 * 64;
            font.set_scale(scale, scale);
            font.set_ppem(PIXEL_SIZE, PIXEL_SIZE);
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
            buffer = buffer.guess_segment_properties();
            let rtl = buffer.get_direction() == Direction::Rtl;
            let output = shape(&font, buffer, &[]);
            let run_start_x = pen_x;
            let mut clusters: BTreeMap<usize, (f32, f32)> = BTreeMap::new();
            for (info, position) in output
                .get_glyph_infos()
                .iter()
                .zip(output.get_glyph_positions())
            {
                let cluster = run.start + info.cluster as usize;
                let advance = position.x_advance as f32 / 64.0;
                let left = pen_x.min(pen_x + advance);
                let right = pen_x.max(pen_x + advance);
                clusters
                    .entry(cluster)
                    .and_modify(|bounds| {
                        bounds.0 = bounds.0.min(left);
                        bounds.1 = bounds.1.max(right);
                    })
                    .or_insert((left, right));
                let doc_byte = map_display_byte(display, cluster);
                glyphs.push(PositionedGlyph {
                    font_index: run.font_index,
                    id: info.codepoint,
                    doc_byte,
                    x: pen_x,
                    x_offset: position.x_offset as f32 / 64.0,
                    y_offset: position.y_offset as f32 / 64.0,
                    invisible: document
                        .get(doc_byte)
                        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r')),
                });
                pen_x += advance;
            }

            let cluster_starts = clusters.keys().copied().collect::<Vec<_>>();
            for (index, cluster) in cluster_starts.iter().copied().enumerate() {
                let next = cluster_starts
                    .get(index + 1)
                    .copied()
                    .unwrap_or(run.start + run.value.len());
                let (left, right) = clusters[&cluster];
                let (leading, trailing) = if rtl { (right, left) } else { (left, right) };
                let doc_start = map_display_byte(display, cluster);
                let doc_end = map_display_byte(display, next);
                let count = doc_end.saturating_sub(doc_start).max(1);
                for offset in 0..=count {
                    let amount = offset as f32 / count as f32;
                    positions.push((doc_start + offset, leading + (trailing - leading) * amount));
                }
            }
            if clusters.is_empty() {
                positions.push((map_display_byte(display, run.start), run_start_x));
            }
        }

        if display.text.is_empty() {
            positions.push((display.display_to_document[0], 0.0));
        }
        (glyphs, pen_x, positions)
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

    pub fn paint_line(
        &mut self,
        context: &egui::Context,
        painter: &egui::Painter,
        origin: Pos2,
        line: &LineLayout,
        color_for_byte: impl Fn(usize) -> Color32,
        byte_is_selected: impl Fn(usize) -> bool,
        invisible_color: Color32,
    ) {
        let baseline = origin.y + line.y + self.ascender();
        for glyph in &line.glyphs {
            if glyph.invisible && !byte_is_selected(glyph.doc_byte) {
                continue;
            }
            let key = GlyphKey {
                font_index: glyph.font_index,
                id: glyph.id,
            };
            self.ensure_glyph(context, key);
            let Some(rasterized) = self.glyphs.get(&key) else {
                continue;
            };
            let Some(texture) = &rasterized.texture else {
                continue;
            };
            let position = Pos2::new(
                origin.x + glyph.x + glyph.x_offset + rasterized.bearing.x,
                baseline - glyph.y_offset - rasterized.bearing.y,
            );
            painter.image(
                texture.id(),
                Rect::from_min_size(position.round(), rasterized.size),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                if glyph.invisible {
                    invisible_color
                } else {
                    color_for_byte(glyph.doc_byte)
                },
            );
        }
    }

    fn ascender(&self) -> f32 {
        self.fonts
            .iter()
            .map(|font| unsafe {
                if ft::FT_Set_Pixel_Sizes(font.face, 0, PIXEL_SIZE) != 0
                    || (*font.face).size.is_null()
                {
                    PIXEL_SIZE as f32
                } else {
                    (*(*font.face).size).metrics.ascender as f32 / 64.0
                }
            })
            .fold(PIXEL_SIZE as f32, f32::max)
    }

    fn ensure_glyph(&mut self, context: &egui::Context, key: GlyphKey) {
        if self.glyphs.contains_key(&key) {
            return;
        }
        let face = self.fonts[key.font_index].face;
        let mut result = RasterizedGlyph {
            texture: None,
            size: Vec2::ZERO,
            bearing: Vec2::ZERO,
        };
        unsafe {
            if ft::FT_Set_Pixel_Sizes(face, 0, PIXEL_SIZE) == 0
                && ft::FT_Load_Glyph(face, key.id, ft::FT_LOAD_DEFAULT as i32) == 0
            {
                let slot = (*face).glyph;
                if ft::FT_Render_Glyph(slot, ft::FT_Render_Mode::FT_RENDER_MODE_NORMAL) == 0 {
                    let bitmap = &(*slot).bitmap;
                    let width = bitmap.width.max(0) as usize;
                    let rows = bitmap.rows.max(0) as usize;
                    let pitch = bitmap.pitch.unsigned_abs() as usize;
                    if !bitmap.buffer.is_null() && width > 0 && rows > 0 {
                        let source = std::slice::from_raw_parts(bitmap.buffer, pitch * rows);
                        let mut rgba = vec![255; width * rows * 4];
                        for row in 0..rows {
                            let source_row = if bitmap.pitch >= 0 {
                                row
                            } else {
                                rows - 1 - row
                            };
                            for column in 0..width {
                                rgba[(row * width + column) * 4 + 3] =
                                    source[source_row * pitch + column];
                            }
                        }
                        let image = egui::ColorImage::from_rgba_unmultiplied([width, rows], &rgba);
                        result.texture = Some(context.load_texture(
                            format!("editor-glyph-{}-{}", key.font_index, key.id),
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                        result.size = Vec2::new(width as f32, rows as f32);
                    }
                    result.bearing =
                        Vec2::new((*slot).bitmap_left as f32, (*slot).bitmap_top as f32);
                }
            }
        }
        self.glyphs.insert(key, result);
    }
}

impl Drop for TextRenderer {
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

fn display_line(bytes: &[u8], document_start: usize, has_newline: bool) -> DisplayLine {
    let mut text = String::new();
    let mut display_to_document = vec![document_start];
    let mut index = 0;
    while index < bytes.len() {
        let (character, consumed) = match std::str::from_utf8(&bytes[index..]) {
            Ok(valid) => (
                valid.chars().next().expect("nonempty UTF-8 text"),
                valid.chars().next().unwrap().len_utf8(),
            ),
            Err(error) if error.valid_up_to() > 0 => {
                let valid = unsafe {
                    std::str::from_utf8_unchecked(&bytes[index..index + error.valid_up_to()])
                };
                let character = valid.chars().next().expect("valid UTF-8 prefix was empty");
                (character, character.len_utf8())
            }
            Err(_) => ('\u{fffd}', 1),
        };
        let display_character = if consumed == 1 {
            match bytes[index] {
                b' ' => '·',
                b'\t' => '⇥',
                b'\r' => '␍',
                _ => character,
            }
        } else {
            character
        };
        append_display_character(
            &mut text,
            &mut display_to_document,
            display_character,
            document_start + index,
            consumed,
        );
        index += consumed;
    }
    if has_newline {
        append_display_character(
            &mut text,
            &mut display_to_document,
            '⏎',
            document_start + bytes.len(),
            1,
        );
    }
    DisplayLine {
        text,
        display_to_document,
    }
}

fn append_display_character(
    text: &mut String,
    display_to_document: &mut Vec<usize>,
    character: char,
    document_start: usize,
    document_len: usize,
) {
    let display_start = text.len();
    text.push(character);
    let display_len = text.len() - display_start;
    for offset in 1..=display_len {
        display_to_document.push(document_start + offset * document_len / display_len);
    }
}

fn map_display_byte(display: &DisplayLine, index: usize) -> usize {
    display
        .display_to_document
        .get(index)
        .copied()
        .unwrap_or_else(|| *display.display_to_document.last().unwrap())
}

fn script_runs(value: &str) -> Vec<TextRun<'_>> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut current = None;
    for (index, character) in value.char_indices() {
        let script = character.script();
        if matches!(script, Script::Common | Script::Inherited | Script::Unknown) {
            continue;
        }
        match current {
            None => current = Some(script),
            Some(active) if active == script => {}
            Some(_) => {
                runs.push(TextRun {
                    value: &value[start..index],
                    start,
                    script: current,
                });
                start = index;
                current = Some(script);
            }
        }
    }
    if !value.is_empty() {
        runs.push(TextRun {
            value: &value[start..],
            start,
            script: current,
        });
    }
    runs
}

fn split_font_runs<'a>(
    run: TextRun<'a>,
    mut font_index_for: impl FnMut(char) -> Option<usize>,
) -> Vec<FontRun<'a>> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut current = None;
    for (index, character) in run.value.char_indices() {
        let font_index = font_index_for(character).or(current).unwrap_or(0);
        match current {
            None => current = Some(font_index),
            Some(active) if active == font_index => {}
            Some(active) => {
                runs.push(FontRun {
                    value: &run.value[start..index],
                    start: run.start + start,
                    script: run.script,
                    font_index: active,
                });
                start = index;
                current = Some(font_index);
            }
        }
    }
    if let Some(font_index) = current {
        runs.push(FontRun {
            value: &run.value[start..],
            start: run.start + start,
            script: run.script,
            font_index,
        });
    }
    runs
}

fn font_paths() -> Vec<PathBuf> {
    FONT_CANDIDATES
        .iter()
        .map(Path::new)
        .filter(|path| path.exists())
        .map(Path::to_owned)
        .collect()
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
