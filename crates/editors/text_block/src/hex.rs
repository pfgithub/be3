use std::ops::Range;

use block_editor_plugin::egui::{
    self, Color32, Event, EventFilter, Key, Modifiers, PointerButton, Pos2, Rect, Sense, Vec2,
};
use text_editor_core::{CursorLeftRightStop, EditorCommand, LRDirection};

use crate::app::TextEditor;

const BYTES_PER_ROW: usize = 16;
const GROUP_SIZE: usize = 8;
const TEXT_SIZE: f32 = 13.0;
const ROW_HEIGHT: f32 = TEXT_SIZE * 1.6;
const PADDING: Vec2 = Vec2::new(12.0, 8.0);
const PAGE_ROWS: usize = 16;

pub(crate) fn intrinsic_size(len: usize, width: f32) -> Vec2 {
    let rows = len.div_ceil(BYTES_PER_ROW).max(1);
    Vec2::new(width, rows as f32 * ROW_HEIGHT + PADDING.y * 2.0)
}

struct HexGeometry {
    char_width: f32,
    hex_x: [f32; BYTES_PER_ROW],
    ascii_x: [f32; BYTES_PER_ROW],
    total_width: f32,
}

impl HexGeometry {
    fn new(char_width: f32) -> Self {
        let offset_end = char_width * 8.0;
        let hex_start = offset_end + char_width * 2.0;
        let mut hex_x = [0.0; BYTES_PER_ROW];
        for (col, x) in hex_x.iter_mut().enumerate() {
            let group_gap = if col >= GROUP_SIZE { char_width } else { 0.0 };
            *x = hex_start + col as f32 * char_width * 3.0 + group_gap;
        }
        let hex_end = hex_x[BYTES_PER_ROW - 1] + char_width * 2.0;
        let ascii_start = hex_end + char_width * 2.0;
        let mut ascii_x = [0.0; BYTES_PER_ROW];
        for (col, x) in ascii_x.iter_mut().enumerate() {
            *x = ascii_start + col as f32 * char_width;
        }
        let total_width = ascii_x[BYTES_PER_ROW - 1] + char_width;
        Self {
            char_width,
            hex_x,
            ascii_x,
            total_width,
        }
    }

    fn region_split(&self) -> f32 {
        let hex_end = self.hex_x[BYTES_PER_ROW - 1] + self.char_width * 2.0;
        (hex_end + self.ascii_x[0]) / 2.0
    }

    fn byte_at(&self, local: Vec2, rows: usize) -> usize {
        let row = ((local.y / ROW_HEIGHT).floor().max(0.0) as usize).min(rows.saturating_sub(1));
        let ascii_region = local.x >= self.region_split();
        let col = if ascii_region {
            nearest_column(&self.ascii_x, self.char_width * 0.5, local.x)
        } else {
            nearest_column(&self.hex_x, self.char_width, local.x)
        };
        row * BYTES_PER_ROW + col
    }
}

fn nearest_column(centers: &[f32; BYTES_PER_ROW], center_offset: f32, x: f32) -> usize {
    let mut best = 0;
    let mut best_distance = f32::MAX;
    for (col, left) in centers.iter().enumerate() {
        let distance = (x - (left + center_offset)).abs();
        if distance < best_distance {
            best_distance = distance;
            best = col;
        }
    }
    best
}

impl TextEditor {
    fn hex_document_len(&self) -> usize {
        self.block.read().map_or(0, |document| document.len())
    }

    fn hex_cursor_range(&self) -> Range<usize> {
        self.core
            .cursor_positions()
            .first()
            .copied()
            .and_then(|cursor| self.core.selection_range(&cursor))
            .unwrap_or(0..0)
    }

    fn select_hex_byte(&mut self, index: usize) {
        let len = self.hex_document_len();
        let index = index.min(len);
        let anchor = self.core.position(index);
        let focus_index = if !self.hex_insert_mode && index < len {
            index + 1
        } else {
            index
        };
        let focus = self.core.position(focus_index);
        self.core
            .execute_command(EditorCommand::SetSelection { anchor, focus });
    }

    fn set_hex_range(&mut self, anchor_byte: usize, target_byte: usize) {
        let len = self.hex_document_len();
        let anchor_byte = anchor_byte.min(len);
        let target_byte = target_byte.min(len);
        let (anchor, focus) = if target_byte >= anchor_byte {
            (
                self.core.position(anchor_byte),
                self.core.position((target_byte + 1).min(len)),
            )
        } else {
            (
                self.core.position((anchor_byte + 1).min(len)),
                self.core.position(target_byte),
            )
        };
        self.core
            .execute_command(EditorCommand::SetSelection { anchor, focus });
    }

    fn commit_hex_byte(&mut self, byte_index: usize, byte: u8) {
        let len = self.hex_document_len();
        let anchor = self.core.position(byte_index.min(len));
        let overwrite_end = if !self.hex_insert_mode && byte_index < len {
            byte_index + 1
        } else {
            byte_index.min(len)
        };
        let focus = self.core.position(overwrite_end);
        self.core
            .execute_command(EditorCommand::SetSelection { anchor, focus });
        self.core
            .execute_command(EditorCommand::InsertText(&[byte]));
        let next = byte_index + 1;
        self.hex_selection_anchor = Some(next);
        if !self.hex_insert_mode {
            self.select_hex_byte(next);
        }
    }

    fn hex_type_nibble(&mut self, digit: u8) {
        let byte_index = self.hex_cursor_range().start;
        match self.hex_pending_nibble.take() {
            None => self.hex_pending_nibble = Some(digit),
            Some(high) => self.commit_hex_byte(byte_index, (high << 4) | digit),
        }
    }

    fn hex_type_text(&mut self, text: &str) {
        for character in text.chars() {
            if let Some(digit) = character.to_digit(16) {
                self.hex_type_nibble(digit as u8);
            }
        }
    }

    fn hex_delete(&mut self, direction: LRDirection) {
        self.hex_pending_nibble = None;
        self.core.execute_command(EditorCommand::Delete {
            direction,
            stop: CursorLeftRightStop::Byte,
        });
        let index = self.hex_cursor_range().start;
        self.hex_selection_anchor = Some(index);
        if !self.hex_insert_mode {
            self.select_hex_byte(index);
        }
    }

    fn hex_select_all(&mut self) {
        self.hex_pending_nibble = None;
        self.core.execute_command(EditorCommand::SelectAll);
    }

    fn hex_copy(&mut self, ui: &egui::Ui, cut: bool) {
        let range = self.hex_cursor_range();
        if range.is_empty() {
            return;
        }
        let Some(bytes) = self
            .block
            .read()
            .map(|document| document.bytes()[range].to_vec())
        else {
            return;
        };
        let hex = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        ui.ctx().copy_text(hex);
        if cut {
            self.hex_pending_nibble = None;
            self.core.execute_command(EditorCommand::Delete {
                direction: LRDirection::Left,
                stop: CursorLeftRightStop::Byte,
            });
            let index = self.hex_cursor_range().start;
            self.hex_selection_anchor = Some(index);
            if !self.hex_insert_mode {
                self.select_hex_byte(index);
            }
        }
    }

    fn hex_paste(&mut self, text: &str) {
        let mut bytes = Vec::new();
        let mut nibble = None;
        for character in text.chars() {
            let Some(digit) = character.to_digit(16) else {
                continue;
            };
            match nibble.take() {
                None => nibble = Some(digit as u8),
                Some(high) => bytes.push((high << 4) | digit as u8),
            }
        }
        if bytes.is_empty() {
            return;
        }
        self.hex_pending_nibble = None;
        let byte_index = self.hex_cursor_range().start;
        let len = self.hex_document_len();
        let anchor = self.core.position(byte_index.min(len));
        let overwrite_end = if self.hex_insert_mode {
            byte_index.min(len)
        } else {
            (byte_index + bytes.len()).min(len)
        };
        let focus = self.core.position(overwrite_end);
        self.core
            .execute_command(EditorCommand::SetSelection { anchor, focus });
        self.core.execute_command(EditorCommand::InsertText(&bytes));
        let next = byte_index + bytes.len();
        self.hex_selection_anchor = Some(next);
        if !self.hex_insert_mode {
            self.select_hex_byte(next);
        }
    }

    fn hex_navigate(&mut self, key: Key, modifiers: Modifiers) {
        let len = self.hex_document_len();
        let current = self.hex_cursor_range().start;
        let target = match key {
            Key::ArrowLeft => current.saturating_sub(1),
            Key::ArrowRight => (current + 1).min(len),
            Key::ArrowUp => current.saturating_sub(BYTES_PER_ROW),
            Key::ArrowDown => (current + BYTES_PER_ROW).min(len),
            Key::Home if modifiers.command => 0,
            Key::Home => current - current % BYTES_PER_ROW,
            Key::End if modifiers.command => len,
            Key::End => (current - current % BYTES_PER_ROW + BYTES_PER_ROW - 1).min(len),
            Key::PageUp => current.saturating_sub(BYTES_PER_ROW * PAGE_ROWS),
            Key::PageDown => (current + BYTES_PER_ROW * PAGE_ROWS).min(len),
            _ => return,
        };
        self.hex_pending_nibble = None;
        if modifiers.shift {
            let anchor = self.hex_selection_anchor.unwrap_or(current);
            self.hex_selection_anchor = Some(anchor);
            self.set_hex_range(anchor, target);
        } else {
            self.hex_selection_anchor = Some(target);
            self.select_hex_byte(target);
        }
    }

    fn hex_keyboard_input(&mut self, ui: &egui::Ui, id: egui::Id) {
        if !ui.memory(|memory| memory.has_focus(id)) {
            return;
        }
        let event_filter = EventFilter {
            tab: false,
            horizontal_arrows: true,
            vertical_arrows: true,
            escape: false,
        };
        ui.memory_mut(|memory| memory.set_focus_lock_filter(id, event_filter));
        let events = ui.input(|input| input.filtered_events(&event_filter));
        for event in events {
            match event {
                Event::Copy => self.hex_copy(ui, false),
                Event::Cut => self.hex_copy(ui, true),
                Event::Paste(text) => self.hex_paste(&text),
                Event::Text(text) => self.hex_type_text(&text),
                Event::Key {
                    key: Key::Backspace,
                    pressed: true,
                    ..
                } => self.hex_delete(LRDirection::Left),
                Event::Key {
                    key: Key::Delete,
                    pressed: true,
                    ..
                } => self.hex_delete(LRDirection::Right),
                Event::Key {
                    key: Key::Insert,
                    pressed: true,
                    ..
                } => {
                    self.hex_insert_mode = !self.hex_insert_mode;
                    self.hex_pending_nibble = None;
                }
                Event::Key {
                    key: Key::A,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.command => self.hex_select_all(),
                Event::Key {
                    key: Key::Z,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.command => {
                    self.hex_pending_nibble = None;
                    self.core.execute_command(if modifiers.shift {
                        EditorCommand::Redo
                    } else {
                        EditorCommand::Undo
                    });
                }
                Event::Key {
                    key: Key::Y,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.command => {
                    self.hex_pending_nibble = None;
                    self.core.execute_command(EditorCommand::Redo);
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => self.hex_navigate(key, modifiers),
                _ => {}
            }
        }
    }

    fn hex_pointer_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        origin: Pos2,
        geometry: &HexGeometry,
        rows: usize,
    ) {
        let (pressed, down) = ui.input(|input| {
            (
                input.pointer.button_pressed(PointerButton::Primary),
                input.pointer.button_down(PointerButton::Primary),
            )
        });
        let Some(pointer) = response.interact_pointer_pos() else {
            if !down {
                self.hex_selection_anchor = None;
            }
            return;
        };
        let len = self.hex_document_len();
        let local = pointer - origin;
        let target = geometry.byte_at(local, rows).min(len);
        if pressed && response.contains_pointer() {
            response.request_focus();
            self.hex_pending_nibble = None;
            self.hex_selection_anchor = Some(target);
            self.select_hex_byte(target);
        } else if self.hex_selection_anchor.is_some() && down {
            if let Some(anchor) = self.hex_selection_anchor {
                self.set_hex_range(anchor, target);
            }
        } else if !down {
            self.hex_selection_anchor = None;
        }
    }

    pub(crate) fn hex_ui(&mut self, ui: &mut egui::Ui) {
        let id = egui::Id::new(("text-editor-hex", self.block.id()));
        let Some(bytes) = self.block.read().map(|document| document.bytes().to_vec()) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let len = bytes.len();
        let font_id = egui::FontId::monospace(TEXT_SIZE);
        let char_width = ui
            .fonts_mut(|fonts| fonts.glyph_width(&font_id, '0'))
            .max(1.0);
        let geometry = HexGeometry::new(char_width);
        let rows = len.div_ceil(BYTES_PER_ROW).max(1);
        let content = Vec2::new(
            geometry.total_width + PADDING.x * 2.0,
            rows as f32 * ROW_HEIGHT + PADDING.y * 2.0,
        );
        let desired = content.max(ui.available_size());
        let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
        let response = ui
            .interact(rect, id, Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Text);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(29, 37, 44));
        let origin = rect.min + PADDING;

        self.hex_pointer_input(ui, &response, origin, &geometry, rows);
        self.hex_keyboard_input(ui, id);

        let range = self.hex_cursor_range();
        let focus = self
            .core
            .cursor_positions()
            .first()
            .copied()
            .and_then(|cursor| self.core.position_index(cursor.pos.focus));

        let clip = ui.clip_rect();
        let first_row = ((clip.top() - origin.y) / ROW_HEIGHT).floor().max(0.0) as usize;
        let last_row =
            (((clip.bottom() - origin.y) / ROW_HEIGHT).ceil().max(0.0) as usize).min(rows);
        let text_color = ui.visuals().text_color();
        let weak_color = ui.visuals().weak_text_color();
        let selection_color = ui.visuals().selection.bg_fill;
        for row in first_row..last_row {
            let row_start = row * BYTES_PER_ROW;
            let row_end = (row_start + BYTES_PER_ROW).min(len);
            paint_row(
                &painter,
                origin,
                row,
                &bytes[row_start..row_end],
                row_start,
                &geometry,
                &font_id,
                text_color,
                weak_color,
                selection_color,
                &range,
            );
        }
        if range.is_empty() {
            if let Some(focus) = focus {
                if focus / BYTES_PER_ROW < rows {
                    paint_caret(&painter, origin, focus, &geometry, text_color);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_row(
    painter: &egui::Painter,
    origin: Pos2,
    row: usize,
    row_bytes: &[u8],
    row_start: usize,
    geometry: &HexGeometry,
    font_id: &egui::FontId,
    text_color: Color32,
    weak_color: Color32,
    selection_color: Color32,
    selected: &Range<usize>,
) {
    let y = origin.y + row as f32 * ROW_HEIGHT;
    painter.text(
        Pos2::new(origin.x, y),
        egui::Align2::LEFT_TOP,
        format!("{row_start:08x}"),
        font_id.clone(),
        weak_color,
    );
    for (col, byte) in row_bytes.iter().enumerate() {
        let index = row_start + col;
        let hex_pos = Pos2::new(origin.x + geometry.hex_x[col], y);
        let ascii_pos = Pos2::new(origin.x + geometry.ascii_x[col], y);
        if selected.contains(&index) {
            painter.rect_filled(
                Rect::from_min_size(hex_pos, Vec2::new(geometry.char_width * 2.0, ROW_HEIGHT)),
                0.0,
                selection_color,
            );
            painter.rect_filled(
                Rect::from_min_size(ascii_pos, Vec2::new(geometry.char_width, ROW_HEIGHT)),
                0.0,
                selection_color,
            );
        }
        painter.text(
            hex_pos,
            egui::Align2::LEFT_TOP,
            format!("{byte:02x}"),
            font_id.clone(),
            text_color,
        );
        let character = if byte.is_ascii_graphic() || *byte == b' ' {
            *byte as char
        } else {
            '.'
        };
        painter.text(
            ascii_pos,
            egui::Align2::LEFT_TOP,
            character,
            font_id.clone(),
            text_color,
        );
    }
}

fn paint_caret(
    painter: &egui::Painter,
    origin: Pos2,
    focus_byte: usize,
    geometry: &HexGeometry,
    color: Color32,
) {
    let row = focus_byte / BYTES_PER_ROW;
    let col = focus_byte % BYTES_PER_ROW;
    let y = origin.y + row as f32 * ROW_HEIGHT;
    let x = origin.x + geometry.hex_x[col];
    painter.rect_filled(
        Rect::from_min_size(Pos2::new(x - 1.0, y), Vec2::new(2.0, ROW_HEIGHT)),
        0.0,
        color,
    );
}
