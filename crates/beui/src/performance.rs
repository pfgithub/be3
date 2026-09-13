use std::collections::VecDeque;
use std::time::{Duration, Instant};

const SAMPLE_CAPACITY: usize = 120;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PerformanceTimings {
    pub total: Duration,
    pub layout: Duration,
    pub interaction: Duration,
    pub paint: Duration,
    pub accessibility: Duration,
    pub other: Duration,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FramePerformance {
    pub timings: PerformanceTimings,
    pub layout_passes: usize,
    pub painted: bool,
    pub nodes: usize,
    pub shapes: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PerformanceSnapshot {
    pub samples: usize,
    pub latest: FramePerformance,
    pub average: PerformanceTimings,
    pub peak: PerformanceTimings,
    pub layout_cache_hits: usize,
    pub paint_cache_hits: usize,
}

#[derive(Default)]
pub(crate) struct PerformanceTracker {
    frames: VecDeque<FramePerformance>,
}

impl PerformanceTracker {
    pub(crate) fn record(&mut self, frame: FramePerformance) {
        if self.frames.len() == SAMPLE_CAPACITY {
            self.frames.pop_front();
        }
        self.frames.push_back(frame);
    }

    pub(crate) fn clear(&mut self) {
        self.frames.clear();
    }

    pub(crate) fn snapshot(&self) -> PerformanceSnapshot {
        let mut snapshot = PerformanceSnapshot {
            samples: self.frames.len(),
            latest: self.frames.back().copied().unwrap_or_default(),
            ..PerformanceSnapshot::default()
        };
        if self.frames.is_empty() {
            return snapshot;
        }
        let mut total = PerformanceTimings::default();
        for frame in &self.frames {
            add(&mut total, frame.timings);
            peak(&mut snapshot.peak, frame.timings);
            snapshot.layout_cache_hits += usize::from(frame.layout_passes == 0);
            snapshot.paint_cache_hits += usize::from(!frame.painted);
        }
        snapshot.average = divide(total, self.frames.len() as u32);
        snapshot
    }
}

pub(crate) struct FrameMeasurement {
    started: Instant,
    pub(crate) timings: PerformanceTimings,
    pub(crate) layout_passes: usize,
    pub(crate) painted: bool,
}

impl FrameMeasurement {
    pub(crate) fn new() -> Self {
        Self {
            started: Instant::now(),
            timings: PerformanceTimings::default(),
            layout_passes: 0,
            painted: false,
        }
    }

    pub(crate) fn measure<R>(slot: &mut Duration, work: impl FnOnce() -> R) -> R {
        let started = Instant::now();
        let result = work();
        *slot += started.elapsed();
        result
    }

    pub(crate) fn finish(mut self, nodes: usize, shapes: usize) -> FramePerformance {
        self.timings.total = self.started.elapsed();
        self.timings.other = self.timings.total.saturating_sub(
            self.timings.layout
                + self.timings.interaction
                + self.timings.paint
                + self.timings.accessibility,
        );
        FramePerformance {
            timings: self.timings,
            layout_passes: self.layout_passes,
            painted: self.painted,
            nodes,
            shapes,
        }
    }
}

fn add(total: &mut PerformanceTimings, value: PerformanceTimings) {
    total.total += value.total;
    total.layout += value.layout;
    total.interaction += value.interaction;
    total.paint += value.paint;
    total.accessibility += value.accessibility;
    total.other += value.other;
}

fn peak(maximum: &mut PerformanceTimings, value: PerformanceTimings) {
    maximum.total = maximum.total.max(value.total);
    maximum.layout = maximum.layout.max(value.layout);
    maximum.interaction = maximum.interaction.max(value.interaction);
    maximum.paint = maximum.paint.max(value.paint);
    maximum.accessibility = maximum.accessibility.max(value.accessibility);
    maximum.other = maximum.other.max(value.other);
}

fn divide(total: PerformanceTimings, count: u32) -> PerformanceTimings {
    PerformanceTimings {
        total: total.total / count,
        layout: total.layout / count,
        interaction: total.interaction / count,
        paint: total.paint / count,
        accessibility: total.accessibility / count,
        other: total.other / count,
    }
}
