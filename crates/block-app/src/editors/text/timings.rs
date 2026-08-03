use std::time::Duration;

#[derive(Clone, Copy, Default)]
pub(super) struct LayoutTimings {
    pub display_lines: Duration,
    pub font_runs: Duration,
    pub shape: Duration,
    pub line_finalize: Duration,
    pub tables: Duration,
}

#[derive(Clone, Copy, Default)]
pub(super) struct PaintTimings {
    pub selection: Duration,
    pub glyphs: Duration,
    pub rasterize: Duration,
    pub glyph_count: usize,
    pub cache_misses: usize,
}

#[derive(Clone, Copy, Default)]
pub(super) struct FrameProfile {
    pub total: Duration,
    pub keyboard: Duration,
    pub toolbar: Duration,
    pub document: Duration,
    pub highlight: Duration,
    pub layout: Duration,
    pub layout_detail: Option<LayoutTimings>,
    pub pointer: Duration,
    pub paint: PaintTimings,
    pub document_bytes: usize,
    pub line_count: usize,
}
