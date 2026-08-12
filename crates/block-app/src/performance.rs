use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    sync::{Mutex, MutexGuard, OnceLock},
    time::{Duration, Instant},
};

use eframe::egui;

const SAMPLE_CAPACITY: usize = 120;

#[derive(Clone)]
enum Value {
    Duration,
    Count(u64),
}

#[derive(Default)]
struct Measurement {
    value: Option<Value>,
    samples: VecDeque<Duration>,
}

#[derive(Default)]
struct Group {
    seen_frame: u64,
    measurements: BTreeMap<String, Measurement>,
}

#[derive(Default)]
struct PerformanceState {
    open: bool,
    frame: u64,
    frame_start: Option<Instant>,
    last_frame: Option<(u64, Duration)>,
    frame_times: VecDeque<Duration>,
    groups: BTreeMap<String, Group>,
}

enum ActiveEntry {
    Group(String),
    Measurement { id: String, start: Instant },
}

thread_local! {
    static ACTIVE: RefCell<Vec<ActiveEntry>> = const { RefCell::new(Vec::new()) };
}

fn state() -> MutexGuard<'static, PerformanceState> {
    static STATE: OnceLock<Mutex<PerformanceState>> = OnceLock::new();
    STATE
        .get_or_init(|| Mutex::new(PerformanceState::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn begin_frame() {
    let mut state = state();
    state.frame = state.frame.wrapping_add(1);
    state.frame_start = Some(Instant::now());
    ACTIVE.with(|active| active.borrow_mut().clear());
}

pub fn end_frame() {
    let mut state = state();
    let elapsed = state.frame_start.take().map(|start| start.elapsed());
    if let Some(elapsed) = elapsed {
        state.last_frame = Some((state.frame, elapsed));
        push_sample(&mut state.frame_times, elapsed);
    }
    drop(state);
    ACTIVE.with(|active| active.borrow_mut().clear());
}

pub fn last_frame() -> Option<(u64, Duration)> {
    state().last_frame
}

pub fn open() {
    state().open = true;
}

pub fn start_group(id: impl Into<String>) {
    let id = id.into();
    let mut state = state();
    let frame = state.frame;
    state.groups.entry(id.clone()).or_default().seen_frame = frame;
    drop(state);
    ACTIVE.with(|active| active.borrow_mut().push(ActiveEntry::Group(id)));
}

pub fn stop_group(id: &str) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(ActiveEntry::Group(active_id)) = active.pop() else {
            debug_assert!(false, "performance group {id:?} was not active");
            return;
        };
        debug_assert_eq!(active_id, id);
    });
}

pub fn start(id: impl Into<String>) {
    ACTIVE.with(|active| {
        active.borrow_mut().push(ActiveEntry::Measurement {
            id: id.into(),
            start: Instant::now(),
        });
    });
}

pub fn stop(id: &str) {
    let completed = ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(ActiveEntry::Measurement {
            id: active_id,
            start,
        }) = active.pop()
        else {
            debug_assert!(false, "performance measurement {id:?} was not active");
            return None;
        };
        debug_assert_eq!(active_id, id);
        let group = active.iter().rev().find_map(|entry| match entry {
            ActiveEntry::Group(group) => Some(group.clone()),
            ActiveEntry::Measurement { .. } => None,
        })?;
        Some((group, start.elapsed()))
    });
    if let Some((group, elapsed)) = completed {
        record_duration_in(&group, id, elapsed);
    }
}

pub fn record_duration(id: impl Into<String>, duration: Duration) {
    let Some(group) = active_group() else {
        return;
    };
    record_duration_in(&group, &id.into(), duration);
}

pub fn increment(id: impl Into<String>) {
    let Some(group) = active_group() else {
        return;
    };
    let id = id.into();
    let mut state = state();
    let measurement = state
        .groups
        .entry(group)
        .or_default()
        .measurements
        .entry(id)
        .or_default();
    let count = match measurement.value {
        Some(Value::Count(count)) => count + 1,
        _ => 1,
    };
    measurement.value = Some(Value::Count(count));
}

pub fn record_count(id: impl Into<String>, count: u64) {
    let Some(group) = active_group() else {
        return;
    };
    state()
        .groups
        .entry(group)
        .or_default()
        .measurements
        .entry(id.into())
        .or_default()
        .value = Some(Value::Count(count));
}

pub struct GroupGuard {
    id: String,
}

impl GroupGuard {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        start_group(id.clone());
        Self { id }
    }
}

impl Drop for GroupGuard {
    fn drop(&mut self) {
        stop_group(&self.id);
    }
}

pub struct MeasurementGuard {
    id: String,
}

impl MeasurementGuard {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        start(id.clone());
        Self { id }
    }
}

impl Drop for MeasurementGuard {
    fn drop(&mut self) {
        stop(&self.id);
    }
}

fn active_group() -> Option<String> {
    ACTIVE.with(|active| {
        active.borrow().iter().rev().find_map(|entry| match entry {
            ActiveEntry::Group(group) => Some(group.clone()),
            ActiveEntry::Measurement { .. } => None,
        })
    })
}

fn record_duration_in(group: &str, id: &str, duration: Duration) {
    let mut state = state();
    let measurement = state
        .groups
        .entry(group.to_owned())
        .or_default()
        .measurements
        .entry(id.to_owned())
        .or_default();
    measurement.value = Some(Value::Duration);
    push_sample(&mut measurement.samples, duration);
}

fn push_sample(samples: &mut VecDeque<Duration>, duration: Duration) {
    if samples.len() == SAMPLE_CAPACITY {
        samples.pop_front();
    }
    samples.push_back(duration);
}

pub fn show(context: &egui::Context) {
    let mut state = state();
    if !state.open {
        return;
    }
    let frame = state.frame;
    let mut open = state.open;
    egui::Window::new("Performance")
        .open(&mut open)
        .resizable(true)
        .default_width(480.0)
        .show(context, |ui| {
            timing_grid(ui, "performance-frame", |ui| {
                timing_row(ui, "Full frame", &state.frame_times);
            });
            for (id, group) in state
                .groups
                .iter()
                .filter(|(_, group)| group.seen_frame == frame)
            {
                egui::CollapsingHeader::new(id)
                    .default_open(true)
                    .show(ui, |ui| {
                        timing_grid(ui, ("performance-group", id), |ui| {
                            for (id, measurement) in &group.measurements {
                                measurement_row(ui, id, measurement);
                            }
                        });
                    });
            }
        });
    state.open = open;
}

fn timing_grid(ui: &mut egui::Ui, id: impl std::hash::Hash, rows: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id)
        .num_columns(4)
        .striped(true)
        .show(ui, |ui| {
            ui.strong("Measurement");
            ui.strong("Current");
            ui.strong("Average");
            ui.strong("Peak");
            ui.end_row();
            rows(ui);
        });
}

fn measurement_row(ui: &mut egui::Ui, id: &str, measurement: &Measurement) {
    match measurement.value {
        Some(Value::Duration) => timing_row(ui, id, &measurement.samples),
        Some(Value::Count(count)) => {
            ui.label(id);
            ui.monospace(count.to_string());
            ui.monospace("-");
            ui.monospace("-");
            ui.end_row();
        }
        None => {}
    }
}

fn timing_row(ui: &mut egui::Ui, id: &str, samples: &VecDeque<Duration>) {
    let current = samples.back().copied().unwrap_or_default();
    let total = samples.iter().copied().sum::<Duration>();
    let average = if samples.is_empty() {
        Duration::ZERO
    } else {
        total / samples.len() as u32
    };
    let peak = samples.iter().copied().max().unwrap_or_default();
    ui.label(id);
    ui.monospace(format_duration(current));
    ui.monospace(format_duration(average));
    ui.monospace(format_duration(peak));
    ui.end_row();
}

fn format_duration(duration: Duration) -> String {
    format!("{:7.3} ms", duration.as_secs_f64() * 1_000.0)
}
