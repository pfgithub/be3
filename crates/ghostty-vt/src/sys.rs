use std::ffi::{c_int, c_void};

pub type Result = c_int;

pub const SUCCESS: Result = 0;
pub const OUT_OF_MEMORY: Result = -1;
pub const INVALID_VALUE: Result = -2;
pub const OUT_OF_SPACE: Result = -3;
pub const NO_VALUE: Result = -4;

pub type Terminal = *mut c_void;
pub type RenderState = *mut c_void;
pub type RowIterator = *mut c_void;
pub type RowCells = *mut c_void;

pub const RENDER_STATE_DATA_ROW_ITERATOR: c_int = 4;
pub const RENDER_STATE_DATA_CURSOR_VISIBLE: c_int = 11;
pub const RENDER_STATE_DATA_CURSOR_VIEWPORT_HAS_VALUE: c_int = 14;
pub const RENDER_STATE_DATA_CURSOR_VIEWPORT_X: c_int = 15;
pub const RENDER_STATE_DATA_CURSOR_VIEWPORT_Y: c_int = 16;

pub const RENDER_STATE_OPTION_DIRTY: c_int = 0;
pub const RENDER_STATE_DIRTY_FALSE: c_int = 0;

pub const ROW_DATA_CELLS: c_int = 3;
pub const ROW_OPTION_DIRTY: c_int = 0;

pub const CELLS_DATA_STYLE: c_int = 2;
pub const CELLS_DATA_GRAPHEMES_LEN: c_int = 3;
pub const CELLS_DATA_BG_COLOR: c_int = 5;
pub const CELLS_DATA_FG_COLOR: c_int = 6;
pub const CELLS_DATA_HAS_STYLING: c_int = 8;
pub const CELLS_DATA_GRAPHEMES_UTF8: c_int = 9;

pub const SCROLL_VIEWPORT_TOP: c_int = 0;
pub const SCROLL_VIEWPORT_BOTTOM: c_int = 1;
pub const SCROLL_VIEWPORT_DELTA: c_int = 2;

pub const STYLE_COLOR_NONE: c_int = 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ColorRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TerminalOptions {
    pub cols: u16,
    pub rows: u16,
    pub max_scrollback: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union ScrollViewportValue {
    pub delta: isize,
    pub row: usize,
    pub padding: [u64; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ScrollViewport {
    pub tag: c_int,
    pub value: ScrollViewportValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RenderStateColors {
    pub size: usize,
    pub background: ColorRgb,
    pub foreground: ColorRgb,
    pub cursor: ColorRgb,
    pub cursor_has_value: bool,
    pub palette: [ColorRgb; 256],
}

impl Default for RenderStateColors {
    fn default() -> Self {
        Self {
            size: size_of::<Self>(),
            background: ColorRgb::default(),
            foreground: ColorRgb::default(),
            cursor: ColorRgb::default(),
            cursor_has_value: false,
            palette: [ColorRgb::default(); 256],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union StyleColorValue {
    pub palette: u8,
    pub rgb: ColorRgb,
    pub padding: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StyleColor {
    pub tag: c_int,
    pub value: StyleColorValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Style {
    pub size: usize,
    pub fg_color: StyleColor,
    pub bg_color: StyleColor,
    pub underline_color: StyleColor,
    pub bold: bool,
    pub italic: bool,
    pub faint: bool,
    pub blink: bool,
    pub inverse: bool,
    pub invisible: bool,
    pub strikethrough: bool,
    pub overline: bool,
    pub underline: c_int,
}

impl Default for Style {
    fn default() -> Self {
        let none = StyleColor {
            tag: STYLE_COLOR_NONE,
            value: StyleColorValue { padding: 0 },
        };
        Self {
            size: size_of::<Self>(),
            fg_color: none,
            bg_color: none,
            underline_color: none,
            bold: false,
            italic: false,
            faint: false,
            blink: false,
            inverse: false,
            invisible: false,
            strikethrough: false,
            overline: false,
            underline: 0,
        }
    }
}

#[repr(C)]
pub struct Buffer {
    pub ptr: *mut u8,
    pub cap: usize,
    pub len: usize,
}

unsafe extern "C" {
    pub fn ghostty_terminal_new(
        allocator: *const c_void,
        terminal: *mut Terminal,
        options: TerminalOptions,
    ) -> Result;
    pub fn ghostty_terminal_free(terminal: Terminal);
    pub fn ghostty_terminal_reset(terminal: Terminal);
    pub fn ghostty_terminal_resize(
        terminal: Terminal,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result;
    pub fn ghostty_terminal_vt_write(terminal: Terminal, data: *const u8, len: usize);
    pub fn ghostty_terminal_scroll_viewport(terminal: Terminal, behavior: ScrollViewport);

    pub fn ghostty_render_state_new(allocator: *const c_void, state: *mut RenderState) -> Result;
    pub fn ghostty_render_state_free(state: RenderState);
    pub fn ghostty_render_state_update(state: RenderState, terminal: Terminal) -> Result;
    pub fn ghostty_render_state_get(state: RenderState, data: c_int, out: *mut c_void) -> Result;
    pub fn ghostty_render_state_set(
        state: RenderState,
        option: c_int,
        value: *const c_void,
    ) -> Result;
    pub fn ghostty_render_state_colors_get(
        state: RenderState,
        out_colors: *mut RenderStateColors,
    ) -> Result;

    pub fn ghostty_render_state_row_iterator_new(
        allocator: *const c_void,
        out_iterator: *mut RowIterator,
    ) -> Result;
    pub fn ghostty_render_state_row_iterator_free(iterator: RowIterator);
    pub fn ghostty_render_state_row_iterator_next(iterator: RowIterator) -> bool;
    pub fn ghostty_render_state_row_get(
        iterator: RowIterator,
        data: c_int,
        out: *mut c_void,
    ) -> Result;
    pub fn ghostty_render_state_row_set(
        iterator: RowIterator,
        option: c_int,
        value: *const c_void,
    ) -> Result;

    pub fn ghostty_render_state_row_cells_new(
        allocator: *const c_void,
        out_cells: *mut RowCells,
    ) -> Result;
    pub fn ghostty_render_state_row_cells_free(cells: RowCells);
    pub fn ghostty_render_state_row_cells_next(cells: RowCells) -> bool;
    pub fn ghostty_render_state_row_cells_get(
        cells: RowCells,
        data: c_int,
        out: *mut c_void,
    ) -> Result;

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn ghostty_type_json() -> *const std::ffi::c_char;
}
