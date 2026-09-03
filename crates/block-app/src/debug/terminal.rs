use std::cell::RefCell;

use eframe::egui;
use ghostty_vt::{Renderer, Rgb, Screen, Terminal};

const FONT_SIZE: f32 = 13.0;
const PADDING: f32 = 6.0;
const DEFAULT_COLS: u16 = 80;
const DEFAULT_ROWS: u16 = 24;
const MAX_SCROLLBACK: usize = 10_000;
const PROMPT: &str = "\x1b[32mblock\x1b[0m:\x1b[34m~\x1b[0m$ ";

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
    terminal: Terminal,
    renderer: Renderer,
    line: String,
    history: Vec<String>,
    recalled: Option<usize>,
    cols: u16,
    rows: u16,
    cell_size: egui::Vec2,
}

impl Session {
    fn new() -> Result<Self, String> {
        let terminal = Terminal::new(DEFAULT_COLS, DEFAULT_ROWS, MAX_SCROLLBACK)
            .map_err(|err| format!("Failed to create the terminal emulator: {err}"))?;
        let renderer =
            Renderer::new().map_err(|err| format!("Failed to create the renderer: {err}"))?;
        let mut session = Self {
            terminal,
            renderer,
            line: String::new(),
            history: Vec::new(),
            recalled: None,
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
            cell_size: egui::vec2(FONT_SIZE * 0.6, FONT_SIZE * 1.2),
        };
        session.banner();
        Ok(session)
    }

    fn banner(&mut self) {
        self.write("\x1b[1mlibghostty-vt demo\x1b[0m\r\n");
        self.write(
            "This window is a terminal emulator with a toy command line attached to it.\r\n\
             Nothing here runs a program: the commands below are all there is.\r\n\
             Type \x1b[1mhelp\x1b[0m for the list.\r\n\r\n",
        );
        self.prompt();
    }

    fn write(&mut self, text: &str) {
        self.terminal.write(text.as_bytes());
    }

    fn write_line(&mut self, text: &str) {
        self.write(text);
        self.write("\r\n");
    }

    fn prompt(&mut self) {
        self.write(PROMPT);
    }

    fn update(&mut self, ui: &mut egui::Ui) {
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
                self.terminal.scroll_by(-delta);
            }
        }

        self.render(ui, rect, &font_id);
    }

    fn handle_input(&mut self, ui: &egui::Ui) {
        for event in ui.ctx().input(|i| i.events.clone()) {
            match event {
                egui::Event::Text(text) => self.insert(&text),
                egui::Event::Paste(text) => self.insert(&text.replace(['\r', '\n'], " ")),
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => self.key(key, modifiers),
                _ => {}
            }
        }
    }

    fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        match key {
            egui::Key::Enter => self.submit(),
            egui::Key::Backspace => self.backspace(),
            egui::Key::ArrowUp => self.recall(-1),
            egui::Key::ArrowDown => self.recall(1),
            egui::Key::C if modifiers.ctrl => {
                self.write_line("^C");
                self.line.clear();
                self.recalled = None;
                self.prompt();
            }
            egui::Key::U if modifiers.ctrl => {
                self.line.clear();
                self.redraw_line();
            }
            egui::Key::L if modifiers.ctrl => {
                self.write("\x1b[2J\x1b[H");
                self.redraw_line();
            }
            _ => {}
        }
    }

    fn insert(&mut self, text: &str) {
        let text = text
            .chars()
            .filter(|character| !character.is_control())
            .collect::<String>();
        if text.is_empty() {
            return;
        }
        self.line.push_str(&text);
        self.write(&text);
    }

    fn backspace(&mut self) {
        if self.line.pop().is_some() {
            self.write("\x08 \x08");
        }
    }

    fn redraw_line(&mut self) {
        let line = std::mem::take(&mut self.line);
        self.write("\r\x1b[K");
        self.prompt();
        self.write(&line);
        self.line = line;
    }

    fn recall(&mut self, direction: isize) {
        if self.history.is_empty() {
            return;
        }
        let last = self.history.len() - 1;
        self.recalled = match (self.recalled, direction) {
            (None, -1) => Some(last),
            (Some(index), -1) => Some(index.saturating_sub(1)),
            (Some(index), _) if index < last => Some(index + 1),
            (Some(_), _) => None,
            (None, _) => None,
        };
        self.line = self
            .recalled
            .map(|index| self.history[index].clone())
            .unwrap_or_default();
        self.redraw_line();
    }

    fn submit(&mut self) {
        self.write("\r\n");
        let line = std::mem::take(&mut self.line);
        self.recalled = None;
        let command = line.trim().to_owned();
        if !command.is_empty() && self.history.last() != Some(&command) {
            self.history.push(command.clone());
        }
        self.run(&command);
        self.prompt();
    }

    fn run(&mut self, line: &str) {
        let (command, argument) = match line.split_once(char::is_whitespace) {
            Some((command, argument)) => (command, argument.trim()),
            None => (line, ""),
        };
        match command {
            "" => {}
            "help" => self.help(),
            "echo" => self.write_line(argument),
            "clear" => self.write("\x1b[2J\x1b[H"),
            "colors" => self.colors(),
            "style" => self.styles(),
            "history" => self.list_history(),
            "about" => self.about(),
            other => self.write_line(&format!(
                "\x1b[31m{other}: not one of the commands this demo knows\x1b[0m"
            )),
        }
    }

    fn help(&mut self) {
        for (command, description) in [
            ("help", "show this list"),
            ("echo TEXT", "write TEXT back out"),
            ("colors", "print the palette the emulator resolves"),
            ("style", "print bold, italic, underlined and inverted text"),
            ("history", "list the commands entered so far"),
            ("clear", "clear the screen"),
            ("about", "what this window is"),
        ] {
            self.write_line(&format!("  \x1b[1m{command:<10}\x1b[0m  {description}"));
        }
        self.write_line("");
        self.write_line("Ctrl+C abandons a line, Ctrl+U clears it, Ctrl+L clears the screen.");
        self.write_line("The arrow keys walk back through the lines already entered.");
    }

    fn colors(&mut self) {
        self.write_line("The sixteen named colors, as foreground and as background:");
        let mut line = String::from("  ");
        for index in 0..16 {
            line.push_str(&format!("\x1b[38;5;{index}m{index:>3}\x1b[0m "));
        }
        self.write_line(&line);
        let mut line = String::from("  ");
        for index in 0..16 {
            line.push_str(&format!("\x1b[48;5;{index}m{index:>3}\x1b[0m "));
        }
        self.write_line(&line);
        self.write_line("");
        self.write_line("A slice of the 256-color cube:");
        for row in 0..6 {
            let mut line = String::from("  ");
            for column in 0..36 {
                let index = 16 + row * 36 + column;
                line.push_str(&format!("\x1b[48;5;{index}m \x1b[0m"));
            }
            self.write_line(&line);
        }
    }

    fn styles(&mut self) {
        self.write_line("  \x1b[1mbold\x1b[0m");
        self.write_line("  \x1b[3mitalic\x1b[0m");
        self.write_line("  \x1b[4munderlined\x1b[0m");
        self.write_line("  \x1b[9mstruck through\x1b[0m");
        self.write_line("  \x1b[7minverted\x1b[0m");
        self.write_line("  \x1b[38;2;255;128;0mtwenty-four bit color\x1b[0m");
    }

    fn list_history(&mut self) {
        if self.history.is_empty() {
            self.write_line("  nothing yet");
            return;
        }
        for (index, entry) in self.history.clone().iter().enumerate() {
            self.write_line(&format!("  {:>3}  {entry}", index + 1));
        }
    }

    fn about(&mut self) {
        self.write_line(
            "The grid above is Ghostty's terminal emulator, libghostty-vt, linked into the app \
             as a static archive.",
        );
        self.write_line(
            "It parses the bytes this command line writes and answers with the cells to draw, \
             which is all a terminal emulator does.",
        );
        self.write_line(
            "Running programs needs a pseudo-terminal, which the browser and Android do not \
             have, so this window has a toy command line in place of a shell.",
        );
    }

    fn render(&mut self, ui: &mut egui::Ui, rect: egui::Rect, font_id: &egui::FontId) {
        let Ok(screen) = self.renderer.update(&mut self.terminal) else {
            return;
        };
        paint(ui, rect, font_id, self.cell_size, screen);
    }
}

fn paint(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    font_id: &egui::FontId,
    cell_size: egui::Vec2,
    screen: &Screen,
) {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, color(screen.background));

    let origin = rect.min + egui::vec2(PADDING, PADDING);
    for (y, row) in screen.rows.iter().enumerate() {
        for (x, cell) in row.cells.iter().enumerate() {
            let position = origin + egui::vec2(x as f32 * cell_size.x, y as f32 * cell_size.y);
            let cell_rect = egui::Rect::from_min_size(position, cell_size);
            if let Some(background) = cell.background {
                painter.rect_filled(cell_rect, 0.0, color(background));
            }
            if cell.text.is_empty() {
                continue;
            }
            let text_color = color(cell.foreground);
            painter.text(
                position,
                egui::Align2::LEFT_TOP,
                &cell.text,
                font_id.clone(),
                text_color,
            );
            if cell.bold {
                painter.text(
                    position + egui::vec2(0.4, 0.0),
                    egui::Align2::LEFT_TOP,
                    &cell.text,
                    font_id.clone(),
                    text_color,
                );
            }
            if cell.underline {
                let y = cell_rect.bottom() - 1.0;
                painter.line_segment(
                    [
                        egui::pos2(cell_rect.left(), y),
                        egui::pos2(cell_rect.right(), y),
                    ],
                    egui::Stroke::new(1.0_f32, text_color),
                );
            }
            if cell.strikethrough {
                let y = cell_rect.center().y;
                painter.line_segment(
                    [
                        egui::pos2(cell_rect.left(), y),
                        egui::pos2(cell_rect.right(), y),
                    ],
                    egui::Stroke::new(1.0_f32, text_color),
                );
            }
        }
    }

    if let Some(cursor) = screen.cursor {
        let position =
            origin + egui::vec2(cursor.x as f32 * cell_size.x, cursor.y as f32 * cell_size.y);
        painter.rect_filled(
            egui::Rect::from_min_size(position, cell_size),
            0.0,
            color(cursor.color).gamma_multiply(0.5),
        );
    }
}

fn color(color: Rgb) -> egui::Color32 {
    egui::Color32::from_rgb(color.r, color.g, color.b)
}
