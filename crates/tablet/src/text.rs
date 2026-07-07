use crate::renderer::{Color, Rect, Scene, Vector};
use freetype::freetype as ft;
use harfbuzz_rs::{shape, Face as HbFace, Font as HbFont, Tag, UnicodeBuffer};
use once_cell::sync::OnceCell;
use std::ffi::CString;
use std::path::Path;
use std::ptr;
use unicode_script::{Script, UnicodeScript};

const TEXT_PIXEL_SIZE: u32 = 18;
const TEXT_SCALE: i32 = (TEXT_PIXEL_SIZE as i32) * 64;

pub(crate) struct TextEngine {
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
    pub(crate) fn new() -> Option<Self> {
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

    pub(crate) fn draw(
        &mut self,
        scene: &mut Scene,
        position: Vector<2, f32>,
        value: &str,
        color: Color,
    ) {
        let baseline = position[1] + self.ascender();
        let mut pen_x = position[0];

        for glyph in self.shape(value) {
            let face = self.fonts[glyph.font_index].face;
            unsafe {
                if ft::FT_Load_Glyph(face, glyph.id, ft::FT_LOAD_DEFAULT as i32) == 0 {
                    let slot = (*face).glyph;
                    if ft::FT_Render_Glyph(slot, ft::FT_Render_Mode::FT_RENDER_MODE_NORMAL) == 0 {
                        let bitmap_x = pen_x + glyph.x_offset + (*slot).bitmap_left as f32;
                        let bitmap_y = baseline - glyph.y_offset - (*slot).bitmap_top as f32;
                        paint_glyph_bitmap(
                            scene,
                            Vector::new(bitmap_x, bitmap_y),
                            &(*slot).bitmap,
                            color,
                        );
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

fn paint_glyph_bitmap(
    scene: &mut Scene,
    position: Vector<2, f32>,
    bitmap: &ft::FT_Bitmap,
    color: Color,
) {
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
    if let Some((uv_min, uv_max)) = scene.add_glyph(Vector::new(width as u32, rows as u32), &pixels)
    {
        scene.push_quad(
            Rect::new(
                Vector::new(position[0].round(), position[1].round()),
                Vector::new(width as f32, rows as f32),
            ),
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
