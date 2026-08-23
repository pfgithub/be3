use std::{
    cell::RefCell,
    io::{Read, Write},
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

use eframe::egui;
use libghostty_vt::{
    key::{self, Key as GhosttyKey},
    render::{CellIterator, Dirty, RenderState, RowIterator},
    style::RgbColor,
    terminal::ScrollViewport,
    Terminal, TerminalOptions,
};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

const FONT_SIZE: f32 = 13.0;
const PADDING: f32 = 6.0;
const DEFAULT_COLS: u16 = 80;
const DEFAULT_ROWS: u16 = 24;
const MAX_SCROLLBACK: usize = 10_000;

#[derive(Default)]
struct TerminalDebugWindow {
    open: bool,
    session: Option<Session>,
    error: Option<String>,
}

thread_local! {
    static STATE: RefCell<TerminalDebugWindow> = RefCell::new(TerminalDebugWindow::default());
}

pub(crate) fn open() {
    STATE.with(|state| state.borrow_mut().open = true);
}

pub(crate) fn show(ctx: &egui::Context) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.open {
            return;
        }

        if state.session.is_none() && state.error.is_none() {
            match Session::new() {
                Ok(session) => state.session = Some(session),
                Err(err) => state.error = Some(err),
            }
        }

        let mut open = state.open;
        egui::Window::new("Terminal")
            .open(&mut open)
            .default_size([820.0, 520.0])
            .resizable(true)
            .show(ctx, |ui| {
                if let Some(error) = state.error.clone() {
                    ui.colored_label(ui.visuals().error_fg_color, &error);
                    if ui.button("Retry").clicked() {
                        state.error = None;
                    }
                    return;
                }
                if let Some(session) = &mut state.session {
                    session.update(ui);
                }
            });
        state.open = open;
        if !open {
            state.session = None;
            state.error = None;
        }
    });
}

struct Session {
    terminal: Terminal<'static, 'static>,
    render_state: RenderState<'static>,
    row_it: RowIterator<'static>,
    cell_it: CellIterator<'static>,
    key_encoder: key::Encoder<'static>,
    key_event: key::Event<'static>,
    writer: Rc<RefCell<Box<dyn Write + Send>>>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    output: mpsc::Receiver<Vec<u8>>,
    response: Vec<u8>,
    cols: u16,
    rows: u16,
    cell_size: egui::Vec2,
}

impl Session {
    fn new() -> Result<Self, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: DEFAULT_ROWS,
                cols: DEFAULT_COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| format!("Failed to open a pseudo-terminal: {err}"))?;

        let mut cmd = CommandBuilder::new_default_prog();
        cmd.env("TERM", "xterm-256color");
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|err| format!("Failed to spawn a shell: {err}"))?;

        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| format!("Failed to open the pseudo-terminal for reading: {err}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|err| format!("Failed to open the pseudo-terminal for writing: {err}"))?;
        let writer = Rc::new(RefCell::new(writer));

        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => return,
                    Ok(len) => {
                        if tx.send(buf[..len].to_vec()).is_err() {
                            return;
                        }
                    }
                }
            }
        });

        let mut terminal = Terminal::new(TerminalOptions {
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
            max_scrollback: MAX_SCROLLBACK,
        })
        .map_err(|err| format!("Failed to create the terminal emulator: {err}"))?;

        terminal
            .on_pty_write({
                let writer = writer.clone();
                move |_terminal, data| {
                    let _ = writer.borrow_mut().write_all(data);
                }
            })
            .map_err(|err| format!("Failed to configure the terminal emulator: {err}"))?;

        let render_state =
            RenderState::new().map_err(|err| format!("Failed to create renderer: {err}"))?;
        let row_it =
            RowIterator::new().map_err(|err| format!("Failed to create row iterator: {err}"))?;
        let cell_it =
            CellIterator::new().map_err(|err| format!("Failed to create cell iterator: {err}"))?;
        let key_encoder =
            key::Encoder::new().map_err(|err| format!("Failed to create key encoder: {err}"))?;
        let key_event =
            key::Event::new().map_err(|err| format!("Failed to create key event: {err}"))?;

        Ok(Self {
            terminal,
            render_state,
            row_it,
            cell_it,
            key_encoder,
            key_event,
            writer,
            master: pair.master,
            child,
            output: rx,
            response: Vec::with_capacity(64),
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
            cell_size: egui::vec2(FONT_SIZE * 0.6, FONT_SIZE * 1.2),
        })
    }

    fn update(&mut self, ui: &mut egui::Ui) {
        while let Ok(chunk) = self.output.try_recv() {
            self.terminal.vt_write(&chunk);
        }

        let font_id = egui::FontId::monospace(FONT_SIZE);
        self.cell_size = ui
            .painter()
            .layout_no_wrap("M".to_owned(), font_id.clone(), egui::Color32::WHITE)
            .size();

        let available = ui.available_size();
        let cols = ((available.x - 2.0 * PADDING) / self.cell_size.x)
            .floor()
            .max(1.0) as u16;
        let rows = ((available.y - 2.0 * PADDING) / self.cell_size.y)
            .floor()
            .max(1.0) as u16;
        if cols != self.cols || rows != self.rows {
            self.cols = cols;
            self.rows = rows;
            let _ =
                self.terminal
                    .resize(cols, rows, self.cell_size.x as u32, self.cell_size.y as u32);
            let _ = self.master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }

        let size = egui::vec2(
            self.cols as f32 * self.cell_size.x + 2.0 * PADDING,
            self.rows as f32 * self.cell_size.y + 2.0 * PADDING,
        );
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        if response.clicked() {
            response.request_focus();
        }
        if response.has_focus() {
            self.handle_input(ui);
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            let delta = (scroll / self.cell_size.y).round() as isize;
            if delta != 0 {
                self.terminal.scroll_viewport(ScrollViewport::Delta(-delta));
            }
        }

        self.render(ui, rect, &font_id);
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }

    fn handle_input(&mut self, ui: &egui::Ui) {
        let events = ui.ctx().input(|i| i.events.clone());
        let mut pending_text = String::new();
        let mut pressed_keys = Vec::new();
        for event in &events {
            match event {
                egui::Event::Text(text) => pending_text.push_str(text),
                egui::Event::Key {
                    key,
                    physical_key,
                    pressed: true,
                    repeat,
                    modifiers,
                } => pressed_keys.push((physical_key.unwrap_or(*key), *modifiers, *repeat)),
                _ => {}
            }
        }

        self.key_encoder.set_options_from_terminal(&self.terminal);
        for (index, (egui_key, modifiers, repeat)) in pressed_keys.iter().enumerate() {
            let Some((ghostty_key, unshifted)) = map_key(*egui_key) else {
                continue;
            };

            let text = if index == 0 && !pending_text.is_empty() {
                Some(std::mem::take(&mut pending_text))
            } else {
                None
            };

            let mut mods = key::Mods::empty();
            if modifiers.shift {
                mods |= key::Mods::SHIFT;
            }
            if modifiers.ctrl {
                mods |= key::Mods::CTRL;
            }
            if modifiers.alt {
                mods |= key::Mods::ALT;
            }
            if modifiers.mac_cmd {
                mods |= key::Mods::SUPER;
            }

            let mut consumed = key::Mods::empty();
            if unshifted != '\0' && mods.contains(key::Mods::SHIFT) {
                consumed |= key::Mods::SHIFT;
            }

            self.key_event
                .set_action(if *repeat {
                    key::Action::Repeat
                } else {
                    key::Action::Press
                })
                .set_key(ghostty_key)
                .set_mods(mods)
                .set_consumed_mods(consumed)
                .set_unshifted_codepoint(unshifted)
                .set_utf8(text.as_deref());

            let _ = self
                .key_encoder
                .encode_to_vec(&self.key_event, &mut self.response);
        }

        if !pending_text.is_empty() {
            self.response.extend_from_slice(pending_text.as_bytes());
        }

        if !self.response.is_empty() {
            let _ = self.writer.borrow_mut().write_all(&self.response);
            self.response.clear();
        }
    }

    fn render(&mut self, ui: &mut egui::Ui, rect: egui::Rect, font_id: &egui::FontId) {
        let painter = ui.painter_at(rect);

        let Ok(snapshot) = self.render_state.update(&self.terminal) else {
            return;
        };
        let Ok(colors) = snapshot.colors() else {
            return;
        };
        painter.rect_filled(rect, 0.0, to_color32(colors.background));

        let Ok(mut row_it) = self.row_it.update(&snapshot) else {
            return;
        };

        let origin = rect.min + egui::vec2(PADDING, PADDING);
        let mut text = String::with_capacity(16);
        let mut y = origin.y;
        while let Some(row) = row_it.next() {
            let mut x = origin.x;
            let Ok(mut cell_it) = self.cell_it.update(row) else {
                break;
            };
            while let Some(cell) = cell_it.next() {
                let cell_rect = egui::Rect::from_min_size(egui::pos2(x, y), self.cell_size);
                let graphemes = cell.graphemes_len().unwrap_or(0);
                let bg = cell.bg_color().ok().flatten();

                if graphemes == 0 {
                    if let Some(bg) = bg {
                        painter.rect_filled(cell_rect, 0.0, to_color32(bg));
                    }
                } else {
                    let mut fg = cell.fg_color().ok().flatten().unwrap_or(colors.foreground);
                    let mut has_bg = bg.is_some();
                    let mut bg = bg.unwrap_or(colors.background);

                    if cell.has_styling().unwrap_or(false) {
                        if let Ok(style) = cell.style() {
                            if style.inverse {
                                std::mem::swap(&mut fg, &mut bg);
                                has_bg = true;
                            }
                        }
                    }

                    if has_bg {
                        painter.rect_filled(cell_rect, 0.0, to_color32(bg));
                    }

                    text.clear();
                    if cell.graphemes_utf8(&mut text).is_ok() {
                        painter.text(
                            cell_rect.min,
                            egui::Align2::LEFT_TOP,
                            &text,
                            font_id.clone(),
                            to_color32(fg),
                        );
                    }
                }

                x += self.cell_size.x;
            }
            let _ = row.set_dirty(false);
            y += self.cell_size.y;
        }

        if snapshot.cursor_visible().unwrap_or(false) {
            if let Ok(Some(cursor)) = snapshot.cursor_viewport() {
                let cursor_color = colors.cursor.unwrap_or(colors.foreground);
                let cursor_rect = egui::Rect::from_min_size(
                    origin
                        + egui::vec2(
                            cursor.x as f32 * self.cell_size.x,
                            cursor.y as f32 * self.cell_size.y,
                        ),
                    self.cell_size,
                );
                painter.rect_filled(
                    cursor_rect,
                    0.0,
                    to_color32(cursor_color).gamma_multiply(0.5),
                );
            }
        }

        let _ = snapshot.set_dirty(Dirty::Clean);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn to_color32(color: RgbColor) -> egui::Color32 {
    egui::Color32::from_rgb(color.r, color.g, color.b)
}

fn map_key(key: egui::Key) -> Option<(GhosttyKey, char)> {
    use egui::Key as EKey;
    Some(match key {
        EKey::A => (GhosttyKey::A, 'a'),
        EKey::B => (GhosttyKey::B, 'b'),
        EKey::C => (GhosttyKey::C, 'c'),
        EKey::D => (GhosttyKey::D, 'd'),
        EKey::E => (GhosttyKey::E, 'e'),
        EKey::F => (GhosttyKey::F, 'f'),
        EKey::G => (GhosttyKey::G, 'g'),
        EKey::H => (GhosttyKey::H, 'h'),
        EKey::I => (GhosttyKey::I, 'i'),
        EKey::J => (GhosttyKey::J, 'j'),
        EKey::K => (GhosttyKey::K, 'k'),
        EKey::L => (GhosttyKey::L, 'l'),
        EKey::M => (GhosttyKey::M, 'm'),
        EKey::N => (GhosttyKey::N, 'n'),
        EKey::O => (GhosttyKey::O, 'o'),
        EKey::P => (GhosttyKey::P, 'p'),
        EKey::Q => (GhosttyKey::Q, 'q'),
        EKey::R => (GhosttyKey::R, 'r'),
        EKey::S => (GhosttyKey::S, 's'),
        EKey::T => (GhosttyKey::T, 't'),
        EKey::U => (GhosttyKey::U, 'u'),
        EKey::V => (GhosttyKey::V, 'v'),
        EKey::W => (GhosttyKey::W, 'w'),
        EKey::X => (GhosttyKey::X, 'x'),
        EKey::Y => (GhosttyKey::Y, 'y'),
        EKey::Z => (GhosttyKey::Z, 'z'),
        EKey::Num0 => (GhosttyKey::Digit0, '0'),
        EKey::Num1 => (GhosttyKey::Digit1, '1'),
        EKey::Num2 => (GhosttyKey::Digit2, '2'),
        EKey::Num3 => (GhosttyKey::Digit3, '3'),
        EKey::Num4 => (GhosttyKey::Digit4, '4'),
        EKey::Num5 => (GhosttyKey::Digit5, '5'),
        EKey::Num6 => (GhosttyKey::Digit6, '6'),
        EKey::Num7 => (GhosttyKey::Digit7, '7'),
        EKey::Num8 => (GhosttyKey::Digit8, '8'),
        EKey::Num9 => (GhosttyKey::Digit9, '9'),
        EKey::Space => (GhosttyKey::Space, ' '),
        EKey::Enter => (GhosttyKey::Enter, '\0'),
        EKey::Tab => (GhosttyKey::Tab, '\0'),
        EKey::Backspace => (GhosttyKey::Backspace, '\0'),
        EKey::Delete => (GhosttyKey::Delete, '\0'),
        EKey::Escape => (GhosttyKey::Escape, '\0'),
        EKey::ArrowUp => (GhosttyKey::ArrowUp, '\0'),
        EKey::ArrowDown => (GhosttyKey::ArrowDown, '\0'),
        EKey::ArrowLeft => (GhosttyKey::ArrowLeft, '\0'),
        EKey::ArrowRight => (GhosttyKey::ArrowRight, '\0'),
        EKey::Home => (GhosttyKey::Home, '\0'),
        EKey::End => (GhosttyKey::End, '\0'),
        EKey::PageUp => (GhosttyKey::PageUp, '\0'),
        EKey::PageDown => (GhosttyKey::PageDown, '\0'),
        EKey::Insert => (GhosttyKey::Insert, '\0'),
        EKey::Minus => (GhosttyKey::Minus, '-'),
        EKey::Equals => (GhosttyKey::Equal, '='),
        EKey::OpenBracket => (GhosttyKey::BracketLeft, '['),
        EKey::CloseBracket => (GhosttyKey::BracketRight, ']'),
        EKey::Backslash => (GhosttyKey::Backslash, '\\'),
        EKey::Semicolon => (GhosttyKey::Semicolon, ';'),
        EKey::Quote => (GhosttyKey::Quote, '\''),
        EKey::Comma => (GhosttyKey::Comma, ','),
        EKey::Period => (GhosttyKey::Period, '.'),
        EKey::Slash => (GhosttyKey::Slash, '/'),
        EKey::Backtick => (GhosttyKey::Backquote, '`'),
        EKey::F1 => (GhosttyKey::F1, '\0'),
        EKey::F2 => (GhosttyKey::F2, '\0'),
        EKey::F3 => (GhosttyKey::F3, '\0'),
        EKey::F4 => (GhosttyKey::F4, '\0'),
        EKey::F5 => (GhosttyKey::F5, '\0'),
        EKey::F6 => (GhosttyKey::F6, '\0'),
        EKey::F7 => (GhosttyKey::F7, '\0'),
        EKey::F8 => (GhosttyKey::F8, '\0'),
        EKey::F9 => (GhosttyKey::F9, '\0'),
        EKey::F10 => (GhosttyKey::F10, '\0'),
        EKey::F11 => (GhosttyKey::F11, '\0'),
        EKey::F12 => (GhosttyKey::F12, '\0'),
        _ => return None,
    })
}
