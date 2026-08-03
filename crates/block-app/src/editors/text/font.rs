use std::{
    collections::{BTreeMap, HashMap},
    ffi::CString,
    ops::Range,
    path::{Path, PathBuf},
    ptr,
    time::{Duration, Instant},
};

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, TextureHandle, Vec2};
use freetype::freetype as ft;
use harfbuzz_rs::{
    shape, Direction, Face as HbFace, Font as HbFont, Owned, Shared, Tag, UnicodeBuffer,
};
use text_editor_core::{
    MarkdownTable, MarkdownTableAlignment, SynHlFontFamily, SynHlStyle, SynHlTextSize,
    SyntaxHighlight,
};
use unicode_script::{Script, UnicodeScript};
use uuid::Uuid;

use super::timings::LayoutTimings;

unsafe extern "C" {
    fn FT_GlyphSlot_Embolden(slot: ft::FT_GlyphSlot);
    fn FT_GlyphSlot_Oblique(slot: ft::FT_GlyphSlot);
}

const BODY_PIXEL_SIZE: u32 = 18;
const CODE_PIXEL_SIZE: u32 = 17;
const INLINE_EMBED_HEIGHT: f32 = 24.0;
const UNAVAILABLE_EMBED_SIZE: Vec2 = Vec2::new(320.0, 120.0);

pub(super) struct TextRenderer {
    library: ft::FT_Library,
    fonts: Vec<FontFace>,
    glyphs: HashMap<GlyphKey, RasterizedGlyph>,
}

pub(super) struct DocumentLayout {
    pub size: Vec2,
    pub lines: Vec<LineLayout>,
    pub positions: Vec<Option<BytePosition>>,
    pub embeds: Vec<EmbedLayout>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ResolvedEmbed {
    pub range: Range<usize>,
    pub id: Uuid,
    pub label: String,
    pub icon: Option<&'static str>,
    pub large: bool,
    pub available: bool,
    pub frame_size: Option<Vec2>,
}

pub(super) struct EmbedLayout {
    pub range: Range<usize>,
    pub id: Uuid,
    pub label: String,
    pub icon: Option<&'static str>,
    pub large: bool,
    pub available: bool,
    pub rect: Rect,
}

pub(super) struct LineLayout {
    pub start: usize,
    pub end: usize,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    baseline: f32,
    glyphs: Vec<PositionedGlyph>,
}

#[derive(Clone, Copy)]
pub(super) struct BytePosition {
    pub line: usize,
    pub x: f32,
}

struct FontFace {
    face: ft::FT_Face,
    hb_fonts: HashMap<u32, Owned<HbFont<'static>>>,
    family: SynHlFontFamily,
    bold: bool,
    italic: bool,
}

impl FontFace {
    fn hb_font(&self, pixel_size: u32) -> &HbFont<'static> {
        &self.hb_fonts[&pixel_size]
    }
}

fn configured_hb_font(face: Shared<HbFace<'static>>, pixel_size: u32) -> Owned<HbFont<'static>> {
    let mut font = HbFont::new(face);
    let scale = pixel_size as i32 * 64;
    font.set_scale(scale, scale);
    font.set_ppem(pixel_size, pixel_size);
    font
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
    style: SynHlStyle,
    pixel_size: u32,
    advance: f32,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct GlyphKey {
    font_index: usize,
    id: u32,
    pixel_size: u32,
    bold: bool,
    italic: bool,
}

struct RasterizedGlyph {
    texture: Option<TextureHandle>,
    size: Vec2,
    bearing: Vec2,
}

pub(super) struct PaintLineProfile {
    pub rasterize: Duration,
    pub glyph_count: usize,
    pub cache_misses: usize,
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
    style: SynHlStyle,
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
        for (path, family, bold, italic) in font_paths() {
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
            if unsafe { ft::FT_Set_Pixel_Sizes(face, 0, BODY_PIXEL_SIZE) } != 0 {
                unsafe { ft::FT_Done_Face(face) };
                continue;
            }
            let Ok(hb_face) = HbFace::from_file(&path, 0) else {
                unsafe { ft::FT_Done_Face(face) };
                continue;
            };
            let hb_face = hb_face.to_shared();
            let hb_fonts = [17, 18, 19, 21, 24, 28, 32]
                .into_iter()
                .map(|pixel_size| (pixel_size, configured_hb_font(hb_face.clone(), pixel_size)))
                .collect();
            fonts.push(FontFace {
                face,
                hb_fonts,
                family,
                bold,
                italic,
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

    pub fn layout_profiled(
        &self,
        bytes: &[u8],
        highlight: &SyntaxHighlight,
        embeds: &[ResolvedEmbed],
    ) -> (DocumentLayout, LayoutTimings) {
        let mut timings = LayoutTimings::default();
        let mut lines = Vec::new();
        let mut embed_layouts = Vec::new();
        let mut positions = vec![None; bytes.len() + 1];
        let mut start = 0;
        let mut line_index = 0;
        let mut y = 0.0;

        loop {
            let newline = bytes[start..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map(|offset| start + offset);
            let end = newline.unwrap_or(bytes.len());
            let line_embeds = embeds
                .iter()
                .filter(|embed| embed.range.start >= start && embed.range.end <= end)
                .collect::<Vec<_>>();
            let display_start = Instant::now();
            let display = display_line(&bytes[start..end], start, newline.is_some(), &line_embeds);
            timings.display_lines += display_start.elapsed();
            let (glyphs, width, line_positions, baseline, height, line_timings) =
                self.layout_line(bytes, &display, highlight);
            timings.font_runs += line_timings.font_runs;
            timings.shape += line_timings.shape;
            timings.line_finalize += line_timings.line_finalize;
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
                y,
                width,
                height,
                baseline,
                glyphs,
            });
            for embed in &line_embeds {
                let Some(left) = positions[embed.range.start] else {
                    continue;
                };
                let Some(right) = positions[embed.range.end] else {
                    continue;
                };
                embed_layouts.push(EmbedLayout {
                    range: embed.range.clone(),
                    id: embed.id,
                    label: embed.label.clone(),
                    icon: embed.icon,
                    large: false,
                    available: embed.available,
                    rect: Rect::from_min_max(
                        Pos2::new(left.x, y + (height - INLINE_EMBED_HEIGHT) * 0.5),
                        Pos2::new(
                            right.x.max(left.x + 1.0),
                            y + (height + INLINE_EMBED_HEIGHT) * 0.5,
                        ),
                    ),
                });
            }
            y += height;
            if let Some(embed) = line_embeds.iter().find(|embed| embed.large) {
                let frame_size = embed.frame_size.unwrap_or(UNAVAILABLE_EMBED_SIZE);
                embed_layouts.push(EmbedLayout {
                    range: embed.range.clone(),
                    id: embed.id,
                    label: embed.label.clone(),
                    icon: embed.icon,
                    large: true,
                    available: embed.available,
                    rect: Rect::from_min_size(Pos2::new(0.0, y), frame_size),
                });
                y += frame_size.y;
            }

            let Some(newline) = newline else {
                break;
            };
            start = newline + 1;
            line_index += 1;
        }

        let tables_start = Instant::now();
        align_markdown_tables(&mut lines, &mut positions, highlight.markdown_tables());
        timings.tables = tables_start.elapsed();
        let width = lines
            .iter()
            .map(|line| line.width)
            .chain(embed_layouts.iter().map(|embed| embed.rect.right()))
            .fold(0.0_f32, f32::max);
        (
            DocumentLayout {
                size: Vec2::new(width + 24.0, y + 16.0),
                lines,
                positions,
                embeds: embed_layouts,
            },
            timings,
        )
    }

    fn layout_line(
        &self,
        document: &[u8],
        display: &DisplayLine,
        highlight: &SyntaxHighlight,
    ) -> (
        Vec<PositionedGlyph>,
        f32,
        Vec<(usize, f32)>,
        f32,
        f32,
        LayoutTimings,
    ) {
        let mut timings = LayoutTimings::default();
        let mut glyphs = Vec::new();
        let mut positions = Vec::new();
        let mut pen_x: f32 = 0.0;

        let font_runs_start = Instant::now();
        let font_runs = self.font_runs(&display.text, display, highlight);
        timings.font_runs = font_runs_start.elapsed();
        for run in font_runs {
            let shape_start = Instant::now();
            let pixel_size = style_pixel_size(run.style);
            let font = self.fonts[run.font_index].hb_font(pixel_size);
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
            let output = shape(font, buffer, &[]);
            timings.shape += shape_start.elapsed();
            let finalize_start = Instant::now();
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
                    style: run.style,
                    pixel_size,
                    advance,
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
            timings.line_finalize += finalize_start.elapsed();
        }

        let finalize_start = Instant::now();
        if display.text.is_empty() {
            positions.push((display.display_to_document[0], 0.0));
        }
        let (baseline, height) = self.line_metrics(&glyphs);
        timings.line_finalize += finalize_start.elapsed();
        (glyphs, pen_x, positions, baseline, height, timings)
    }

    fn font_runs<'a>(
        &self,
        value: &'a str,
        display: &DisplayLine,
        highlight: &SyntaxHighlight,
    ) -> Vec<FontRun<'a>> {
        script_runs(value)
            .into_iter()
            .flat_map(|run| {
                split_font_runs(run, |display_byte, character| {
                    let style = highlight.style_at(map_display_byte(display, display_byte));
                    (
                        self.font_index_for(character, style.family, style.bold, style.italic),
                        style,
                    )
                })
            })
            .collect()
    }

    fn font_index_for(
        &self,
        character: char,
        family: SynHlFontFamily,
        bold: bool,
        italic: bool,
    ) -> Option<usize> {
        self.fonts
            .iter()
            .position(|font| {
                font.family == family
                    && font.bold == bold
                    && font.italic == italic
                    && unsafe { ft::FT_Get_Char_Index(font.face, character as ft::FT_ULong) != 0 }
            })
            .or_else(|| {
                self.fonts.iter().position(|font| {
                    font.family == family
                        && unsafe {
                            ft::FT_Get_Char_Index(font.face, character as ft::FT_ULong) != 0
                        }
                })
            })
            .or_else(|| {
                self.fonts.iter().position(|font| unsafe {
                    ft::FT_Get_Char_Index(font.face, character as ft::FT_ULong) != 0
                })
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
    ) -> PaintLineProfile {
        let mut profile = PaintLineProfile {
            rasterize: Duration::ZERO,
            glyph_count: line.glyphs.len(),
            cache_misses: 0,
        };
        let baseline = origin.y + line.y + line.baseline;
        for glyph in &line.glyphs {
            if glyph.invisible && !byte_is_selected(glyph.doc_byte) {
                continue;
            }
            let key = GlyphKey {
                font_index: glyph.font_index,
                id: glyph.id,
                pixel_size: glyph.pixel_size,
                bold: glyph.style.bold && !self.fonts[glyph.font_index].bold,
                italic: glyph.style.italic && !self.fonts[glyph.font_index].italic,
            };
            if !self.glyphs.contains_key(&key) {
                profile.cache_misses += 1;
                let rasterize_start = Instant::now();
                self.ensure_glyph(context, key);
                profile.rasterize += rasterize_start.elapsed();
            }
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
            let color = color_for_byte(glyph.doc_byte);
            let left = origin.x + glyph.x.min(glyph.x + glyph.advance);
            let right = origin.x + glyph.x.max(glyph.x + glyph.advance);
            if glyph.style.underline {
                let y = baseline + (glyph.pixel_size as f32 * 0.12).max(1.0);
                painter.line_segment(
                    [Pos2::new(left, y), Pos2::new(right, y)],
                    Stroke::new((glyph.pixel_size as f32 / 16.0).max(1.0), color),
                );
            }
            if glyph.style.strikethrough {
                let y = baseline - glyph.pixel_size as f32 * 0.32;
                painter.line_segment(
                    [Pos2::new(left, y), Pos2::new(right, y)],
                    Stroke::new((glyph.pixel_size as f32 / 16.0).max(1.0), color),
                );
            }
        }
        profile
    }

    fn line_metrics(&self, glyphs: &[PositionedGlyph]) -> (f32, f32) {
        let mut ascender = BODY_PIXEL_SIZE as f32;
        let mut descender = BODY_PIXEL_SIZE as f32 * 0.3;
        for glyph in glyphs {
            let face = self.fonts[glyph.font_index].face;
            unsafe {
                if ft::FT_Set_Pixel_Sizes(face, 0, glyph.pixel_size) == 0 && !(*face).size.is_null()
                {
                    ascender = ascender.max((*(*face).size).metrics.ascender as f32 / 64.0);
                    descender = descender.max(-(*(*face).size).metrics.descender as f32 / 64.0);
                }
            }
        }
        (ascender + 3.0, ascender + descender + 7.0)
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
            if ft::FT_Set_Pixel_Sizes(face, 0, key.pixel_size) == 0
                && ft::FT_Load_Glyph(face, key.id, ft::FT_LOAD_DEFAULT as i32) == 0
            {
                let slot = (*face).glyph;
                if key.bold {
                    FT_GlyphSlot_Embolden(slot);
                }
                if key.italic {
                    FT_GlyphSlot_Oblique(slot);
                }
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
                            format!(
                                "editor-glyph-{}-{}-{}-{}-{}",
                                key.font_index, key.id, key.pixel_size, key.bold, key.italic
                            ),
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

fn align_markdown_tables(
    lines: &mut [LineLayout],
    positions: &mut [Option<BytePosition>],
    tables: &[MarkdownTable],
) {
    for table in tables {
        let rows = table
            .rows
            .iter()
            .filter_map(|row| {
                let line_index = positions
                    .get(row.range.start)
                    .and_then(|position| *position)?
                    .line;
                let line = lines.get(line_index)?;
                let cells = row
                    .cells
                    .iter()
                    .filter_map(|cell| {
                        let start = byte_x(positions, cell.start, line_index)?;
                        let end = byte_x(positions, cell.end, line_index)?;
                        Some((cell.clone(), start, end))
                    })
                    .collect::<Vec<_>>();
                (!cells.is_empty()).then_some((line_index, line.start, line.end, line.width, cells))
            })
            .collect::<Vec<_>>();
        if rows.is_empty() {
            continue;
        }

        let column_count = rows
            .iter()
            .map(|(_, _, _, _, cells)| cells.len())
            .max()
            .unwrap_or(0);
        let mut column_widths = vec![0.0_f32; column_count];
        let mut gap_widths = vec![0.0_f32; column_count.saturating_sub(1)];
        let mut prefix_width = 0.0_f32;
        for (_, _, _, _, cells) in &rows {
            prefix_width = prefix_width.max(cells[0].1);
            for (column, (_, start, end)) in cells.iter().enumerate() {
                column_widths[column] = column_widths[column].max((end - start).max(0.0));
                if let Some((_, next_start, _)) = cells.get(column + 1) {
                    gap_widths[column] = gap_widths[column].max((next_start - end).max(0.0));
                }
            }
        }

        for (line_index, line_start, line_end, natural_line_width, cells) in rows {
            let mut segments = Vec::new();
            let first_start = cells[0].0.start;
            segments.push(LayoutSegment {
                position_range: line_start..first_start,
                glyph_range: line_start..first_start,
                offset: prefix_width - cells[0].1,
            });

            let mut target_x = prefix_width;
            for (column, (cell, natural_start, natural_end)) in cells.iter().enumerate() {
                let cell_width = (natural_end - natural_start).max(0.0);
                let spare = (column_widths[column] - cell_width).max(0.0);
                let alignment = table
                    .alignments
                    .get(column)
                    .copied()
                    .unwrap_or(MarkdownTableAlignment::Left);
                let leading = match alignment {
                    MarkdownTableAlignment::Left => 0.0,
                    MarkdownTableAlignment::Center => spare / 2.0,
                    MarkdownTableAlignment::Right => spare,
                };
                segments.push(LayoutSegment {
                    position_range: cell.start..cell.end.saturating_add(1),
                    glyph_range: cell.clone(),
                    offset: target_x + leading - natural_start,
                });
                target_x += column_widths[column];

                if let Some((next, next_start, _)) = cells.get(column + 1) {
                    segments.push(LayoutSegment {
                        position_range: cell.end.saturating_add(1)..next.start,
                        glyph_range: cell.end..next.start,
                        offset: target_x - natural_end,
                    });
                    let natural_gap = (next_start - natural_end).max(0.0);
                    target_x += gap_widths[column].max(natural_gap);
                }
            }

            let (_, _, last_natural_end) = cells.last().expect("table row has a cell");
            let last_end = cells.last().expect("table row has a cell").0.end;
            let suffix_offset = target_x - last_natural_end;
            segments.push(LayoutSegment {
                position_range: last_end.saturating_add(1)..line_end.saturating_add(1),
                glyph_range: last_end..line_end.saturating_add(1),
                offset: suffix_offset,
            });

            for segment in &segments {
                for position in positions
                    .get_mut(segment.position_range.clone())
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    if position.line == line_index {
                        position.x += segment.offset;
                    }
                }
            }
            let line = &mut lines[line_index];
            for glyph in &mut line.glyphs {
                if let Some(segment) = segments
                    .iter()
                    .find(|segment| segment.glyph_range.contains(&glyph.doc_byte))
                {
                    glyph.x += segment.offset;
                }
            }
            line.width = natural_line_width + suffix_offset;
        }
    }
}

struct LayoutSegment {
    position_range: std::ops::Range<usize>,
    glyph_range: std::ops::Range<usize>,
    offset: f32,
}

fn byte_x(positions: &[Option<BytePosition>], byte: usize, line: usize) -> Option<f32> {
    positions
        .get(byte)
        .and_then(|position| *position)
        .filter(|position| position.line == line)
        .map(|position| position.x)
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

fn display_line(
    bytes: &[u8],
    document_start: usize,
    has_newline: bool,
    embeds: &[&ResolvedEmbed],
) -> DisplayLine {
    let mut text = String::new();
    let mut display_to_document = vec![document_start];
    let mut index = 0;
    while index < bytes.len() {
        let document_index = document_start + index;
        if let Some(embed) = embeds
            .iter()
            .find(|embed| embed.range.start == document_index)
        {
            append_display_text(
                &mut text,
                &mut display_to_document,
                &if embed.icon.is_some() {
                    format!("   {} ", embed.label)
                } else {
                    format!(" {} ", embed.label)
                },
                embed.range.start,
                embed.range.len(),
            );
            index = embed.range.end - document_start;
            continue;
        }
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

fn append_display_text(
    text: &mut String,
    display_to_document: &mut Vec<usize>,
    value: &str,
    document_start: usize,
    document_len: usize,
) {
    let display_start = text.len();
    text.push_str(value);
    let display_len = text.len() - display_start;
    for offset in 1..=display_len {
        display_to_document.push(document_start + offset * document_len / display_len);
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
    mut font_and_style_for: impl FnMut(usize, char) -> (Option<usize>, SynHlStyle),
) -> Vec<FontRun<'a>> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut current: Option<(usize, SynHlStyle)> = None;
    for (index, character) in run.value.char_indices() {
        let (font_index, style) = font_and_style_for(run.start + index, character);
        let font_index = font_index
            .or_else(|| current.map(|(font_index, _)| font_index))
            .unwrap_or(0);
        match current {
            None => current = Some((font_index, style)),
            Some(active) if active == (font_index, style) => {}
            Some((active_font, active_style)) => {
                runs.push(FontRun {
                    value: &run.value[start..index],
                    start: run.start + start,
                    script: run.script,
                    font_index: active_font,
                    style: active_style,
                });
                start = index;
                current = Some((font_index, style));
            }
        }
    }
    if let Some((font_index, style)) = current {
        runs.push(FontRun {
            value: &run.value[start..],
            start: run.start + start,
            script: run.script,
            font_index,
            style,
        });
    }
    runs
}

fn style_pixel_size(style: SynHlStyle) -> u32 {
    if style.family == SynHlFontFamily::Monospace {
        return CODE_PIXEL_SIZE;
    }
    match style.size {
        SynHlTextSize::Body => BODY_PIXEL_SIZE,
        SynHlTextSize::Heading(1) => 32,
        SynHlTextSize::Heading(2) => 28,
        SynHlTextSize::Heading(3) => 24,
        SynHlTextSize::Heading(4) => 21,
        SynHlTextSize::Heading(5) => 19,
        SynHlTextSize::Heading(_) => 18,
    }
}

fn font_paths() -> Vec<(PathBuf, SynHlFontFamily, bool, bool)> {
    FONT_CANDIDATES
        .iter()
        .map(|path| (*path, SynHlFontFamily::Proportional, false, false))
        .chain(
            BOLD_FONT_CANDIDATES
                .iter()
                .map(|path| (*path, SynHlFontFamily::Proportional, true, false)),
        )
        .chain(
            ITALIC_FONT_CANDIDATES
                .iter()
                .map(|path| (*path, SynHlFontFamily::Proportional, false, true)),
        )
        .chain(
            BOLD_ITALIC_FONT_CANDIDATES
                .iter()
                .map(|path| (*path, SynHlFontFamily::Proportional, true, true)),
        )
        .chain(
            MONOSPACE_FONT_CANDIDATES
                .iter()
                .map(|path| (*path, SynHlFontFamily::Monospace, false, false)),
        )
        .chain(
            MONOSPACE_BOLD_FONT_CANDIDATES
                .iter()
                .map(|path| (*path, SynHlFontFamily::Monospace, true, false)),
        )
        .chain(
            MONOSPACE_ITALIC_FONT_CANDIDATES
                .iter()
                .map(|path| (*path, SynHlFontFamily::Monospace, false, true)),
        )
        .chain(
            MONOSPACE_BOLD_ITALIC_FONT_CANDIDATES
                .iter()
                .map(|path| (*path, SynHlFontFamily::Monospace, true, true)),
        )
        .map(|(path, family, bold, italic)| (Path::new(path), family, bold, italic))
        .filter(|(path, _, _, _)| path.exists())
        .map(|(path, family, bold, italic)| (path.to_owned(), family, bold, italic))
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

const BOLD_FONT_CANDIDATES: &[&str] = &[
    "C:\\Windows\\Fonts\\verdanab.ttf",
    "C:\\Windows\\Fonts\\segoeuib.ttf",
    "C:\\Windows\\Fonts\\arialbd.ttf",
    "/System/Library/Fonts/Supplemental/Verdana Bold.ttf",
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
];

const ITALIC_FONT_CANDIDATES: &[&str] = &[
    "C:\\Windows\\Fonts\\verdanai.ttf",
    "C:\\Windows\\Fonts\\segoeuii.ttf",
    "C:\\Windows\\Fonts\\ariali.ttf",
    "/System/Library/Fonts/Supplemental/Verdana Italic.ttf",
    "/System/Library/Fonts/Supplemental/Arial Italic.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Oblique.ttf",
];

const BOLD_ITALIC_FONT_CANDIDATES: &[&str] = &[
    "C:\\Windows\\Fonts\\verdanaz.ttf",
    "C:\\Windows\\Fonts\\segoeuiz.ttf",
    "C:\\Windows\\Fonts\\arialbi.ttf",
    "/System/Library/Fonts/Supplemental/Verdana Bold Italic.ttf",
    "/System/Library/Fonts/Supplemental/Arial Bold Italic.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-BoldOblique.ttf",
];

const MONOSPACE_FONT_CANDIDATES: &[&str] = &[
    "C:\\Windows\\Fonts\\consola.ttf",
    "C:\\Windows\\Fonts\\cour.ttf",
    "/System/Library/Fonts/Menlo.ttc",
    "/System/Library/Fonts/Monaco.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationMono-Regular.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
];

const MONOSPACE_BOLD_FONT_CANDIDATES: &[&str] = &[
    "C:\\Windows\\Fonts\\consolab.ttf",
    "C:\\Windows\\Fonts\\courbd.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
];

const MONOSPACE_ITALIC_FONT_CANDIDATES: &[&str] = &[
    "C:\\Windows\\Fonts\\consolai.ttf",
    "C:\\Windows\\Fonts\\couri.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Oblique.ttf",
];

const MONOSPACE_BOLD_ITALIC_FONT_CANDIDATES: &[&str] = &[
    "C:\\Windows\\Fonts\\consolaz.ttf",
    "C:\\Windows\\Fonts\\courbi.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-BoldOblique.ttf",
];
