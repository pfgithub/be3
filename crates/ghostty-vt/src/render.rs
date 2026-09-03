use std::ffi::c_void;
use std::ptr;

use crate::sys;
use crate::terminal::{check, Error, Terminal};

pub type Rgb = sys::ColorRgb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub x: u16,
    pub y: u16,
    pub color: Rgb,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub foreground: Rgb,
    pub background: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl Cell {
    fn reset(&mut self, foreground: Rgb) {
        self.text.clear();
        self.foreground = foreground;
        self.background = None;
        self.bold = false;
        self.italic = false;
        self.underline = false;
        self.strikethrough = false;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub cells: Vec<Cell>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Screen {
    pub background: Rgb,
    pub foreground: Rgb,
    pub cursor: Option<Cursor>,
    pub rows: Vec<Row>,
}

impl Screen {
    pub fn text(&self) -> String {
        let mut text = String::new();
        for (index, row) in self.rows.iter().enumerate() {
            if index > 0 {
                text.push('\n');
            }
            for cell in &row.cells {
                text.push_str(if cell.text.is_empty() {
                    " "
                } else {
                    &cell.text
                });
            }
            while text.ends_with(' ') {
                text.pop();
            }
        }
        text
    }
}

pub struct Renderer {
    state: sys::RenderState,
    rows: sys::RowIterator,
    cells: sys::RowCells,
    screen: Screen,
}

impl Renderer {
    pub fn new() -> Result<Self, Error> {
        let mut renderer = Self {
            state: ptr::null_mut(),
            rows: ptr::null_mut(),
            cells: ptr::null_mut(),
            screen: Screen::default(),
        };
        check(unsafe { sys::ghostty_render_state_new(ptr::null(), &mut renderer.state) })?;
        check(unsafe {
            sys::ghostty_render_state_row_iterator_new(ptr::null(), &mut renderer.rows)
        })?;
        check(unsafe {
            sys::ghostty_render_state_row_cells_new(ptr::null(), &mut renderer.cells)
        })?;
        Ok(renderer)
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn update(&mut self, terminal: &mut Terminal) -> Result<&Screen, Error> {
        let state = self.state;
        let mut iterator = self.rows;
        let cells = self.cells;
        check(unsafe { sys::ghostty_render_state_update(state, terminal.handle()) })?;

        let mut colors = sys::RenderStateColors::default();
        check(unsafe { sys::ghostty_render_state_colors_get(state, &mut colors) })?;

        let screen = &mut self.screen;
        screen.background = colors.background;
        screen.foreground = colors.foreground;
        screen.cursor = cursor(state, &colors);

        check(unsafe {
            sys::ghostty_render_state_get(
                state,
                sys::RENDER_STATE_DATA_ROW_ITERATOR,
                ptr::from_mut(&mut iterator).cast(),
            )
        })?;

        let mut row_count = 0;
        while unsafe { sys::ghostty_render_state_row_iterator_next(iterator) } {
            if screen.rows.len() == row_count {
                screen.rows.push(Row::default());
            }
            let row = &mut screen.rows[row_count];
            row_count += 1;

            let mut row_cells = cells;
            check(unsafe {
                sys::ghostty_render_state_row_get(
                    iterator,
                    sys::ROW_DATA_CELLS,
                    ptr::from_mut(&mut row_cells).cast(),
                )
            })?;

            let mut cell_count = 0;
            while unsafe { sys::ghostty_render_state_row_cells_next(row_cells) } {
                if row.cells.len() == cell_count {
                    row.cells.push(Cell::default());
                }
                read_cell(row_cells, &colors, &mut row.cells[cell_count]);
                cell_count += 1;
            }
            row.cells.truncate(cell_count);

            let clean = false;
            let _ = unsafe {
                sys::ghostty_render_state_row_set(
                    iterator,
                    sys::ROW_OPTION_DIRTY,
                    ptr::from_ref(&clean).cast(),
                )
            };
        }
        screen.rows.truncate(row_count);

        let clean = sys::RENDER_STATE_DIRTY_FALSE;
        let _ = unsafe {
            sys::ghostty_render_state_set(
                state,
                sys::RENDER_STATE_OPTION_DIRTY,
                ptr::from_ref(&clean).cast(),
            )
        };

        Ok(&self.screen)
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            sys::ghostty_render_state_row_cells_free(self.cells);
            sys::ghostty_render_state_row_iterator_free(self.rows);
            sys::ghostty_render_state_free(self.state);
        }
    }
}

fn cursor(state: sys::RenderState, colors: &sys::RenderStateColors) -> Option<Cursor> {
    if !state_flag(state, sys::RENDER_STATE_DATA_CURSOR_VISIBLE)
        || !state_flag(state, sys::RENDER_STATE_DATA_CURSOR_VIEWPORT_HAS_VALUE)
    {
        return None;
    }
    let mut x: u16 = 0;
    let mut y: u16 = 0;
    let position = unsafe {
        sys::ghostty_render_state_get(
            state,
            sys::RENDER_STATE_DATA_CURSOR_VIEWPORT_X,
            ptr::from_mut(&mut x).cast(),
        ) == sys::SUCCESS
            && sys::ghostty_render_state_get(
                state,
                sys::RENDER_STATE_DATA_CURSOR_VIEWPORT_Y,
                ptr::from_mut(&mut y).cast(),
            ) == sys::SUCCESS
    };
    if !position {
        return None;
    }
    Some(Cursor {
        x,
        y,
        color: if colors.cursor_has_value {
            colors.cursor
        } else {
            colors.foreground
        },
    })
}

fn state_flag(state: sys::RenderState, data: std::ffi::c_int) -> bool {
    let mut value = false;
    let result =
        unsafe { sys::ghostty_render_state_get(state, data, ptr::from_mut(&mut value).cast()) };
    result == sys::SUCCESS && value
}

fn read_cell(cells: sys::RowCells, colors: &sys::RenderStateColors, cell: &mut Cell) {
    cell.reset(colors.foreground);

    let mut length: u32 = 0;
    if cell_get(
        cells,
        sys::CELLS_DATA_GRAPHEMES_LEN,
        ptr::from_mut(&mut length).cast(),
    ) && length > 0
    {
        read_text(cells, &mut cell.text);
    }

    let mut foreground = sys::ColorRgb::default();
    if cell_get(
        cells,
        sys::CELLS_DATA_FG_COLOR,
        ptr::from_mut(&mut foreground).cast(),
    ) {
        cell.foreground = foreground;
    }

    let mut background = sys::ColorRgb::default();
    if cell_get(
        cells,
        sys::CELLS_DATA_BG_COLOR,
        ptr::from_mut(&mut background).cast(),
    ) {
        cell.background = Some(background);
    }

    let mut styled = false;
    if !cell_get(
        cells,
        sys::CELLS_DATA_HAS_STYLING,
        ptr::from_mut(&mut styled).cast(),
    ) || !styled
    {
        return;
    }

    let mut style = sys::Style::default();
    if !cell_get(
        cells,
        sys::CELLS_DATA_STYLE,
        ptr::from_mut(&mut style).cast(),
    ) {
        return;
    }
    cell.bold = style.bold;
    cell.italic = style.italic;
    cell.underline = style.underline != 0;
    cell.strikethrough = style.strikethrough;
    if style.inverse {
        let foreground = cell.background.unwrap_or(colors.background);
        cell.background = Some(cell.foreground);
        cell.foreground = foreground;
    }
}

fn read_text(cells: sys::RowCells, text: &mut String) {
    let mut storage = [0u8; 64];
    let mut buffer = sys::Buffer {
        ptr: storage.as_mut_ptr(),
        cap: storage.len(),
        len: 0,
    };
    let result = unsafe {
        sys::ghostty_render_state_row_cells_get(
            cells,
            sys::CELLS_DATA_GRAPHEMES_UTF8,
            ptr::from_mut(&mut buffer).cast::<c_void>(),
        )
    };
    if result == sys::SUCCESS {
        text.push_str(&String::from_utf8_lossy(&storage[..buffer.len]));
        return;
    }
    if result != sys::OUT_OF_SPACE {
        return;
    }

    let mut storage = vec![0u8; buffer.len];
    let mut buffer = sys::Buffer {
        ptr: storage.as_mut_ptr(),
        cap: storage.len(),
        len: 0,
    };
    let result = unsafe {
        sys::ghostty_render_state_row_cells_get(
            cells,
            sys::CELLS_DATA_GRAPHEMES_UTF8,
            ptr::from_mut(&mut buffer).cast::<c_void>(),
        )
    };
    if result == sys::SUCCESS {
        text.push_str(&String::from_utf8_lossy(&storage[..buffer.len]));
    }
}

fn cell_get(cells: sys::RowCells, data: std::ffi::c_int, out: *mut c_void) -> bool {
    unsafe { sys::ghostty_render_state_row_cells_get(cells, data, out) == sys::SUCCESS }
}
