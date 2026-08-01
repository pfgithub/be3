use std::time::Duration;

use eframe::egui;

pub(super) const SAMPLE_CAPACITY: usize = 120;

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

pub(super) struct TextProfiler {
    open: bool,
    samples: [FrameProfile; SAMPLE_CAPACITY],
    next_sample: usize,
    sample_count: usize,
}

impl Default for TextProfiler {
    fn default() -> Self {
        Self {
            open: false,
            samples: [FrameProfile::default(); SAMPLE_CAPACITY],
            next_sample: 0,
            sample_count: 0,
        }
    }
}

impl TextProfiler {
    pub fn toggle(&mut self, ui: &mut egui::Ui) {
        ui.toggle_value(&mut self.open, "Performance");
    }

    pub fn record(&mut self, sample: FrameProfile) {
        self.samples[self.next_sample] = sample;
        self.next_sample = (self.next_sample + 1) % SAMPLE_CAPACITY;
        self.sample_count = (self.sample_count + 1).min(SAMPLE_CAPACITY);
    }

    pub fn show(&mut self, context: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("Text editor performance")
            .open(&mut open)
            .resizable(true)
            .show(context, |ui| {
                ui.label(format!(
                    "Rolling window: {} / {} frames",
                    self.sample_count, SAMPLE_CAPACITY
                ));
                ui.label("Times are CPU time spent by the editor UI; nested rows are included in their parent.");
                ui.add_space(4.0);
                egui::Grid::new("text-editor-performance-timings")
                    .num_columns(4)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Stage");
                        ui.strong("Current");
                        ui.strong("Average");
                        ui.strong("Peak");
                        ui.end_row();
                        self.timing_row(ui, "Frame total", |sample| sample.total);
                        self.timing_row(ui, "  Keyboard input", |sample| sample.keyboard);
                        self.timing_row(ui, "  Toolbar", |sample| sample.toolbar);
                        self.timing_row(ui, "  Document read + copy", |sample| sample.document);
                        self.timing_row(ui, "  Syntax highlight", |sample| sample.highlight);
                        self.timing_row(ui, "  Layout total", |sample| sample.layout);
                        self.conditional_timing_row(ui, "    Display-line conversion", |sample| {
                            sample.layout_detail.map(|detail| detail.display_lines)
                        });
                        self.conditional_timing_row(
                            ui,
                            "    Font/style run detection",
                            |sample| sample.layout_detail.map(|detail| detail.font_runs),
                        );
                        self.conditional_timing_row(ui, "    HarfBuzz shaping", |sample| {
                            sample.layout_detail.map(|detail| detail.shape)
                        });
                        self.conditional_timing_row(
                            ui,
                            "    Line positions + metrics",
                            |sample| sample.layout_detail.map(|detail| detail.line_finalize),
                        );
                        self.conditional_timing_row(
                            ui,
                            "    Markdown table alignment",
                            |sample| sample.layout_detail.map(|detail| detail.tables),
                        );
                        self.timing_row(ui, "  Pointer hit testing", |sample| sample.pointer);
                        self.timing_row(ui, "  Selection + cursor paint", |sample| {
                            sample.paint.selection
                        });
                        self.timing_row(ui, "  Glyph paint", |sample| sample.paint.glyphs);
                        self.conditional_timing_row(ui, "    Glyph rasterization", |sample| {
                            (sample.paint.cache_misses != 0).then_some(sample.paint.rasterize)
                        });
                    });
                if let Some(current) = self.current() {
                    ui.add_space(6.0);
                    ui.monospace(format!(
                        "Document: {} bytes, {} lines | glyphs visited: {} | cache misses: {}",
                        current.document_bytes,
                        current.line_count,
                        current.paint.glyph_count,
                        current.paint.cache_misses
                    ));
                }
                if ui.button("Reset samples").clicked() {
                    self.next_sample = 0;
                    self.sample_count = 0;
                }
            });
        self.open = open;
    }

    fn timing_row(&self, ui: &mut egui::Ui, name: &str, value: impl Fn(&FrameProfile) -> Duration) {
        let current = self.current().map(&value).unwrap_or_default();
        let (average, peak) = self.aggregate(value);
        ui.label(name);
        ui.monospace(format_duration(current));
        ui.monospace(format_duration(average));
        ui.monospace(format_duration(peak));
        ui.end_row();
    }

    fn conditional_timing_row(
        &self,
        ui: &mut egui::Ui,
        name: &str,
        value: impl Fn(&FrameProfile) -> Option<Duration>,
    ) {
        let current = self.recent_samples().find_map(&value).unwrap_or_default();
        let (average, peak) = self.aggregate_optional(value);
        ui.label(name);
        ui.monospace(format_duration(current));
        ui.monospace(format_duration(average));
        ui.monospace(format_duration(peak));
        ui.end_row();
    }

    fn current(&self) -> Option<&FrameProfile> {
        (self.sample_count != 0).then(|| {
            let index = (self.next_sample + SAMPLE_CAPACITY - 1) % SAMPLE_CAPACITY;
            &self.samples[index]
        })
    }

    fn recent_samples(&self) -> impl Iterator<Item = &FrameProfile> {
        (0..self.sample_count).map(|offset| {
            let index = (self.next_sample + SAMPLE_CAPACITY - 1 - offset) % SAMPLE_CAPACITY;
            &self.samples[index]
        })
    }

    fn aggregate(&self, value: impl Fn(&FrameProfile) -> Duration) -> (Duration, Duration) {
        if self.sample_count == 0 {
            return (Duration::ZERO, Duration::ZERO);
        }
        let mut total = Duration::ZERO;
        let mut peak = Duration::ZERO;
        for sample in &self.samples[..self.sample_count] {
            let duration = value(sample);
            total += duration;
            peak = peak.max(duration);
        }
        (total / self.sample_count as u32, peak)
    }

    fn aggregate_optional(
        &self,
        value: impl Fn(&FrameProfile) -> Option<Duration>,
    ) -> (Duration, Duration) {
        let mut total = Duration::ZERO;
        let mut peak = Duration::ZERO;
        let mut count = 0;
        for sample in &self.samples[..self.sample_count] {
            let Some(duration) = value(sample) else {
                continue;
            };
            total += duration;
            peak = peak.max(duration);
            count += 1;
        }
        if count == 0 {
            (Duration::ZERO, Duration::ZERO)
        } else {
            (total / count, peak)
        }
    }
}

fn format_duration(duration: Duration) -> String {
    format!("{:7.3} ms", duration.as_secs_f64() * 1_000.0)
}
