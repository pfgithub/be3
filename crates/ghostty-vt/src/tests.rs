use crate::{Renderer, Screen, Terminal};

mod renders_written_text;
mod reports_cursor_position;
mod resolves_styles_and_colors;
mod struct_layouts_match_the_library;

fn render(cols: u16, rows: u16, data: &str) -> Screen {
    let mut terminal = Terminal::new(cols, rows, 100).unwrap();
    terminal.write(data.as_bytes());
    let mut renderer = Renderer::new().unwrap();
    renderer.update(&mut terminal).unwrap().clone()
}
