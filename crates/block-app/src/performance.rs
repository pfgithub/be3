use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    sync::{Mutex, MutexGuard, OnceLock},
    time::{Duration, Instant},
};

use eframe::egui;

const SAMPLE_CAPACITY: usize = 120;
const CAUSE_CAPACITY: usize = 8;

#[derive(Clone)]
pub struct LastFrame {
    pub number: u64,
    pub duration: Duration,
    pub causes: Vec<String>,
}

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
    frame_causes: Vec<String>,
    last_frame: Option<LastFrame>,
    frame_times: VecDeque<Duration>,
    groups: BTreeMap<String, Group>,
}

thread_local! {
    static ACTIVE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn state() -> MutexGuard<'static, PerformanceState> {
    static STATE: OnceLock<Mutex<PerformanceState>> = OnceLock::new();
    STATE
        .get_or_init(|| Mutex::new(PerformanceState::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn begin_frame(context: &egui::Context) {
    let causes = repaint_causes(context);
    let mut state = state();
    state.frame = state.frame.wrapping_add(1);
    state.frame_start = Some(Instant::now());
    state.frame_causes = causes;
    ACTIVE.with(|active| active.borrow_mut().clear());
}

pub fn end_frame() {
    let mut state = state();
    let elapsed = state.frame_start.take().map(|start| start.elapsed());
    if let Some(elapsed) = elapsed {
        state.last_frame = Some(LastFrame {
            number: state.frame,
            duration: elapsed,
            causes: state.frame_causes.clone(),
        });
        push_sample(&mut state.frame_times, elapsed);
    }
    drop(state);
    ACTIVE.with(|active| active.borrow_mut().clear());
}

pub fn last_frame() -> Option<LastFrame> {
    state().last_frame.clone()
}

fn repaint_causes(context: &egui::Context) -> Vec<String> {
    let mut causes: Vec<String> = Vec::new();
    for cause in context.repaint_causes() {
        let cause = format_cause(&cause);
        if !causes.contains(&cause) {
            causes.push(cause);
        }
        if causes.len() == CAUSE_CAPACITY {
            break;
        }
    }
    causes
}

fn format_cause(cause: &egui::RepaintCause) -> String {
    let file = shorten_path(cause.file);
    let line = cause.line;
    if cause.reason.is_empty() {
        format!("{file}:{line}")
    } else {
        format!("{file}:{line} ({})", cause.reason)
    }
}

fn shorten_path(path: &str) -> String {
    let mut components: Vec<&str> = path.split(['/', '\\']).collect();
    if components.len() > 3 {
        components = components.split_off(components.len() - 3);
    }
    components.join("/")
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
    ACTIVE.with(|active| active.borrow_mut().push(id));
}

pub fn stop_group(id: &str) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(active_id) = active.pop() else {
            debug_assert!(false, "performance group {id:?} was not active");
            return;
        };
        debug_assert_eq!(active_id, id);
    });
}

pub fn record_duration(id: impl Into<String>, duration: Duration) {
    let Some(group) = active_group() else {
        return;
    };
    record_duration_in(&group, &id.into(), duration);
}

pub fn record_count(id: impl Into<String>, count: u64) {
    let Some(group) = active_group() else {
        return;
    };
    record_count_in(&group, &id.into(), count);
}

pub fn record_group_duration(group: &str, id: &str, duration: Duration) {
    record_duration_in(group, id, duration);
}

pub fn record_group_count(group: &str, id: &str, count: u64) {
    record_count_in(group, id, count);
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

fn active_group() -> Option<String> {
    ACTIVE.with(|active| active.borrow().last().cloned())
}

fn record_duration_in(group: &str, id: &str, duration: Duration) {
    let mut state = state();
    let frame = state.frame;
    let group = state.groups.entry(group.to_owned()).or_default();
    group.seen_frame = frame;
    let measurement = group.measurements.entry(id.to_owned()).or_default();
    measurement.value = Some(Value::Duration);
    push_sample(&mut measurement.samples, duration);
}

fn record_count_in(group: &str, id: &str, count: u64) {
    let mut state = state();
    let frame = state.frame;
    let group = state.groups.entry(group.to_owned()).or_default();
    group.seen_frame = frame;
    group.measurements.entry(id.to_owned()).or_default().value = Some(Value::Count(count));
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
