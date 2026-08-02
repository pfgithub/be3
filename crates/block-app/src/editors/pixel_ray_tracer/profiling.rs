use std::time::Duration;

use eframe::egui;

const SAMPLE_CAPACITY: usize = 120;

#[derive(Clone, Copy, Default)]
struct FrameProfile {
    total: Duration,
    lighting: Option<Duration>,
    view_rays: Option<Duration>,
}

pub(super) struct RayTracerProfiler {
    open: bool,
    samples: [FrameProfile; SAMPLE_CAPACITY],
    next_sample: usize,
    sample_count: usize,
    pending_lighting: Option<Duration>,
    pending_view_rays: Option<Duration>,
    lighting_cache_hits: u64,
    lighting_cache_misses: u64,
    ray_cache_hits: u64,
    ray_cache_misses: u64,
}

impl Default for RayTracerProfiler {
    fn default() -> Self {
        Self {
            open: false,
            samples: [FrameProfile::default(); SAMPLE_CAPACITY],
            next_sample: 0,
            sample_count: 0,
            pending_lighting: None,
            pending_view_rays: None,
            lighting_cache_hits: 0,
            lighting_cache_misses: 0,
            ray_cache_hits: 0,
            ray_cache_misses: 0,
        }
    }
}

impl RayTracerProfiler {
    pub fn toggle(&mut self, ui: &mut egui::Ui) {
        ui.toggle_value(&mut self.open, "Performance");
    }

    pub fn lighting_hit(&mut self) {
        self.lighting_cache_hits += 1;
    }

    pub fn lighting_miss(&mut self) {
        self.lighting_cache_misses += 1;
    }

    pub fn ray_hit(&mut self) {
        self.ray_cache_hits += 1;
    }

    pub fn ray_miss(&mut self) {
        self.ray_cache_misses += 1;
    }

    pub fn lighting_completed(&mut self, duration: Duration) {
        self.pending_lighting = Some(duration);
    }

    pub fn ray_completed(&mut self, duration: Duration) {
        self.pending_view_rays = Some(duration);
    }

    pub fn record_frame(&mut self, total: Duration) {
        self.samples[self.next_sample] = FrameProfile {
            total,
            lighting: self.pending_lighting.take(),
            view_rays: self.pending_view_rays.take(),
        };
        self.next_sample = (self.next_sample + 1) % SAMPLE_CAPACITY;
        self.sample_count = (self.sample_count + 1).min(SAMPLE_CAPACITY);
    }

    pub fn show(&mut self, context: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("Pixel ray tracer performance")
            .open(&mut open)
            .resizable(true)
            .show(context, |ui| {
                ui.label(format!(
                    "Rolling window: {} / {} frames",
                    self.sample_count, SAMPLE_CAPACITY
                ));
                ui.label("Trace jobs run off the UI thread; completed job time appears on the frame that receives it.");
                ui.add_space(4.0);
                egui::Grid::new("pixel-ray-tracer-performance-timings")
                    .num_columns(4)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Stage");
                        ui.strong("Current");
                        ui.strong("Average");
                        ui.strong("Peak");
                        ui.end_row();
                        self.timing_row(ui, "Editor frame", |sample| Some(sample.total));
                        self.timing_row(ui, "Lighting trace", |sample| sample.lighting);
                        self.timing_row(ui, "View-ray trace", |sample| sample.view_rays);
                    });
                ui.add_space(6.0);
                ui.monospace(format!(
                    "Lighting cache: {} hits / {} misses\nView-ray cache: {} hits / {} misses",
                    self.lighting_cache_hits,
                    self.lighting_cache_misses,
                    self.ray_cache_hits,
                    self.ray_cache_misses
                ));
                if ui.button("Reset samples").clicked() {
                    self.next_sample = 0;
                    self.sample_count = 0;
                    self.lighting_cache_hits = 0;
                    self.lighting_cache_misses = 0;
                    self.ray_cache_hits = 0;
                    self.ray_cache_misses = 0;
                }
            });
        self.open = open;
    }

    fn timing_row(
        &self,
        ui: &mut egui::Ui,
        name: &str,
        value: impl Fn(&FrameProfile) -> Option<Duration>,
    ) {
        let current = self.recent_samples().find_map(&value).unwrap_or_default();
        let mut total = Duration::ZERO;
        let mut peak = Duration::ZERO;
        let mut count = 0;
        for sample in self.recent_samples() {
            let Some(duration) = value(sample) else {
                continue;
            };
            total += duration;
            peak = peak.max(duration);
            count += 1;
        }
        let average = if count == 0 {
            Duration::ZERO
        } else {
            total / count
        };
        ui.label(name);
        ui.monospace(format_duration(current));
        ui.monospace(format_duration(average));
        ui.monospace(format_duration(peak));
        ui.end_row();
    }

    fn recent_samples(&self) -> impl Iterator<Item = &FrameProfile> {
        (0..self.sample_count).map(|offset| {
            let index = (self.next_sample + SAMPLE_CAPACITY - 1 - offset) % SAMPLE_CAPACITY;
            &self.samples[index]
        })
    }
}

fn format_duration(duration: Duration) -> String {
    format!("{:7.3} ms", duration.as_secs_f64() * 1_000.0)
}
