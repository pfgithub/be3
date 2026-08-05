use std::time::{SystemTime, UNIX_EPOCH};

use block_client::{
    blocks::calendar::{Calendar, CalendarEvent, CalendarOperation},
    BlockClient, BlockHandle,
};
use eframe::egui;
use egui_material_icons::{
    icons::{
        ICON_ADD, ICON_CALENDAR_MONTH, ICON_CALENDAR_VIEW_DAY, ICON_CALENDAR_VIEW_MONTH,
        ICON_CALENDAR_VIEW_WEEK, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_CLOSE, ICON_DELETE,
        ICON_SAVE,
    },
    MaterialIcon,
};
use uuid::Uuid;

use super::{
    BlockEditor, CreatableEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess,
    EditorAction, EditorKind,
};

const SECONDS_PER_DAY: i64 = 86_400;
const DIRECT_EDITOR_WIDTH: f32 = 880.0;
const MONTH_GRID_HEIGHT: f32 = 600.0;
const MONTH_CELL_HEIGHT: f32 = 92.0;
const MONTH_COLUMN_WIDTH: f32 = 120.0;
const TIMELINE_VIEW_HEIGHT: f32 = 600.0;
const TIMELINE_HEADER_HEIGHT: f32 = 22.0;
const HOUR_HEIGHT: f32 = 40.0;
const HOUR_GUTTER_WIDTH: f32 = 52.0;
const TIMELINE_CONTENT_HEIGHT: f32 = HOUR_HEIGHT * 24.0;
const MIN_EVENT_HEIGHT: f32 = 16.0;

const WEEKDAY_ABBR: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const WEEKDAY_FULL: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

impl EditorKind for CalendarEditor {
    type Block = Calendar;

    const DISPLAY_NAME: &'static str = "Calendar";
    const ICON: MaterialIcon = ICON_CALENDAR_MONTH;

    fn open(_client: &BlockClient, block: BlockHandle<Calendar>) -> Self {
        Self::new(block)
    }
}

impl CreatableEditor for CalendarEditor {
    fn create(client: &BlockClient) -> Self {
        Self::new(client.create_block(Calendar::new()))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CalendarView {
    Day,
    Week,
    Month,
}

/// The date and time fields the event form edits, kept separate from the
/// event's unix-seconds representation so partially typed values do not have
/// to round-trip through it.
#[derive(Clone, Copy)]
struct DateTimeFields {
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
}

impl DateTimeFields {
    fn from_unix(seconds: i64) -> Self {
        let days = seconds.div_euclid(SECONDS_PER_DAY);
        let time_of_day = seconds.rem_euclid(SECONDS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        Self {
            year,
            month,
            day,
            hour: (time_of_day / 3600) as u8,
            minute: ((time_of_day % 3600) / 60) as u8,
        }
    }

    fn to_unix(self) -> i64 {
        let month = self.month.clamp(1, 12);
        let day = self.day.clamp(1, days_in_month(self.year, month));
        days_from_civil(self.year, month, day) * SECONDS_PER_DAY
            + self.hour as i64 * 3600
            + self.minute as i64 * 60
    }
}

struct EventForm {
    editing_id: Option<Uuid>,
    title: String,
    start: DateTimeFields,
    end: DateTimeFields,
}

impl EventForm {
    fn new_at(day: i64, hour: u8) -> Self {
        let start_seconds = day * SECONDS_PER_DAY + hour as i64 * 3600;
        Self {
            editing_id: None,
            title: String::new(),
            start: DateTimeFields::from_unix(start_seconds),
            end: DateTimeFields::from_unix(start_seconds + 3600),
        }
    }

    fn edit(event: &CalendarEvent) -> Self {
        Self {
            editing_id: Some(event.id),
            title: event.title.clone(),
            start: DateTimeFields::from_unix(event.start),
            end: DateTimeFields::from_unix(event.end),
        }
    }
}

pub(super) struct CalendarEditor {
    block: BlockHandle<Calendar>,
    view: CalendarView,
    /// The focused day: the shown day in day view, a day in the shown week
    /// in week view, or a day in the shown month in month view.
    anchor_day: i64,
    form: Option<EventForm>,
}

impl CalendarEditor {
    fn new(block: BlockHandle<Calendar>) -> Self {
        Self {
            block,
            view: CalendarView::Month,
            anchor_day: today_days_since_epoch(),
            form: None,
        }
    }

    fn go_previous(&mut self) {
        self.anchor_day = match self.view {
            CalendarView::Day => self.anchor_day - 1,
            CalendarView::Week => self.anchor_day - 7,
            CalendarView::Month => {
                let (year, month, _) = civil_from_days(self.anchor_day);
                let (year, month) = if month == 1 {
                    (year - 1, 12)
                } else {
                    (year, month - 1)
                };
                days_from_civil(year, month, 1)
            }
        };
    }

    fn go_next(&mut self) {
        self.anchor_day = match self.view {
            CalendarView::Day => self.anchor_day + 1,
            CalendarView::Week => self.anchor_day + 7,
            CalendarView::Month => {
                let (year, month, _) = civil_from_days(self.anchor_day);
                let (year, month) = if month == 12 {
                    (year + 1, 1)
                } else {
                    (year, month + 1)
                };
                days_from_civil(year, month, 1)
            }
        };
    }

    fn go_today(&mut self) {
        self.anchor_day = today_days_since_epoch();
    }

    fn header_label(&self) -> String {
        match self.view {
            CalendarView::Day => {
                let (year, month, day) = civil_from_days(self.anchor_day);
                format!(
                    "{}, {} {}, {}",
                    WEEKDAY_FULL[weekday_from_days(self.anchor_day) as usize],
                    MONTH_NAMES[(month - 1) as usize],
                    day,
                    year
                )
            }
            CalendarView::Week => {
                let start = week_start(self.anchor_day);
                let end = start + 6;
                let (start_year, start_month, start_day) = civil_from_days(start);
                let (end_year, end_month, end_day) = civil_from_days(end);
                if start_year == end_year && start_month == end_month {
                    format!(
                        "{} {} - {}, {}",
                        MONTH_NAMES[(start_month - 1) as usize],
                        start_day,
                        end_day,
                        start_year
                    )
                } else {
                    format!(
                        "{} {} - {} {}, {}",
                        MONTH_NAMES[(start_month - 1) as usize],
                        start_day,
                        MONTH_NAMES[(end_month - 1) as usize],
                        end_day,
                        end_year
                    )
                }
            }
            CalendarView::Month => {
                let (year, month, _) = civil_from_days(self.anchor_day);
                format!("{} {}", MONTH_NAMES[(month - 1) as usize], year)
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button(ICON_CHEVRON_LEFT).clicked() {
                self.go_previous();
            }
            if ui.button("Today").clicked() {
                self.go_today();
            }
            if ui.button(ICON_CHEVRON_RIGHT).clicked() {
                self.go_next();
            }
            ui.separator();
            ui.strong(self.header_label());
            ui.separator();
            for (view, icon, label) in [
                (CalendarView::Day, ICON_CALENDAR_VIEW_DAY, "Day"),
                (CalendarView::Week, ICON_CALENDAR_VIEW_WEEK, "Week"),
                (CalendarView::Month, ICON_CALENDAR_VIEW_MONTH, "Month"),
            ] {
                if ui
                    .selectable_label(self.view == view, format!("{} {label}", icon.codepoint))
                    .clicked()
                {
                    self.view = view;
                }
            }
            ui.separator();
            if ui.button(format!("{} Event", ICON_ADD.codepoint)).clicked() {
                self.form = Some(EventForm::new_at(self.anchor_day, 9));
            }
        });
    }

    fn month_view(&mut self, ui: &mut egui::Ui, events: &[CalendarEvent]) {
        let (year, month, _) = civil_from_days(self.anchor_day);
        let first_of_month = days_from_civil(year, month, 1);
        let grid_start = first_of_month - weekday_from_days(first_of_month) as i64;
        let today = today_days_since_epoch();
        let column_width = ui.available_width().max(MONTH_COLUMN_WIDTH * 7.0) / 7.0;

        ui.horizontal(|ui| {
            for name in WEEKDAY_ABBR {
                ui.add_sized(
                    [column_width, 20.0],
                    egui::Label::new(egui::RichText::new(name).strong()),
                );
            }
        });

        egui::Grid::new(("calendar-month-grid", self.block.id()))
            .num_columns(7)
            .spacing(egui::Vec2::splat(2.0))
            .show(ui, |ui| {
                for week in 0..6 {
                    for weekday in 0..7 {
                        let day = grid_start + week * 7 + weekday;
                        let (day_year, day_month, day_number) = civil_from_days(day);
                        let in_month = day_year == year && day_month == month;
                        let day_start_secs = day * SECONDS_PER_DAY;
                        let day_end_secs = day_start_secs + SECONDS_PER_DAY;
                        let day_events: Vec<&CalendarEvent> = events
                            .iter()
                            .filter(|event| {
                                event.start < day_end_secs && event.end > day_start_secs
                            })
                            .collect();
                        self.month_cell(
                            ui,
                            column_width,
                            day,
                            day_number,
                            in_month,
                            day == today,
                            &day_events,
                        );
                    }
                    ui.end_row();
                }
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn month_cell(
        &mut self,
        ui: &mut egui::Ui,
        width: f32,
        day: i64,
        day_number: u8,
        in_month: bool,
        is_today: bool,
        events: &[&CalendarEvent],
    ) {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, MONTH_CELL_HEIGHT), egui::Sense::click());
        let visuals = ui.visuals();
        let fill = if is_today {
            visuals.selection.bg_fill.gamma_multiply(0.3)
        } else if in_month {
            visuals.extreme_bg_color
        } else {
            visuals.faint_bg_color
        };
        ui.painter().rect(
            rect,
            4.0,
            fill,
            visuals.widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let text_color = if in_month {
            visuals.text_color()
        } else {
            visuals.weak_text_color()
        };
        ui.painter().text(
            rect.left_top() + egui::vec2(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            day_number.to_string(),
            egui::FontId::proportional(13.0),
            text_color,
        );

        let mut event_clicked = false;
        let mut y = rect.top() + 22.0;
        let max_visible = 3;
        for (index, event) in events.iter().enumerate() {
            if index >= max_visible {
                ui.painter().text(
                    egui::pos2(rect.left() + 6.0, y),
                    egui::Align2::LEFT_TOP,
                    format!("+{} more", events.len() - index),
                    egui::FontId::proportional(10.0),
                    visuals.weak_text_color(),
                );
                break;
            }
            let event_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left() + 3.0, y),
                egui::vec2(width - 6.0, 15.0),
            );
            let id = ui.id().with(("calendar-month-event", event.id));
            let event_response = ui.interact(event_rect, id, egui::Sense::click());
            ui.painter().rect_filled(
                event_rect,
                2.0,
                visuals
                    .selection
                    .bg_fill
                    .gamma_multiply(if event_response.hovered() { 1.0 } else { 0.85 }),
            );
            ui.painter().text(
                event_rect.left_top() + egui::vec2(3.0, 1.0),
                egui::Align2::LEFT_TOP,
                truncated(&event.title, 14),
                egui::FontId::proportional(10.0),
                visuals.strong_text_color(),
            );
            if event_response.clicked() {
                self.form = Some(EventForm::edit(event));
                event_clicked = true;
            }
            y += 17.0;
        }

        if response.clicked() && !event_clicked {
            self.form = Some(EventForm::new_at(day, 9));
        }
    }

    fn timeline_view(
        &mut self,
        ui: &mut egui::Ui,
        events: &[CalendarEvent],
        first_day: i64,
        num_days: i64,
    ) {
        let gutter = HOUR_GUTTER_WIDTH;
        let available_width = ui.available_width().max(gutter + num_days as f32 * 90.0);
        let column_width = (available_width - gutter) / num_days as f32;

        ui.horizontal(|ui| {
            ui.add_space(gutter);
            let today = today_days_since_epoch();
            for day_index in 0..num_days {
                let day = first_day + day_index;
                let (_, _, day_number) = civil_from_days(day);
                let label = format!(
                    "{} {}",
                    WEEKDAY_ABBR[weekday_from_days(day) as usize],
                    day_number
                );
                let mut text = egui::RichText::new(label);
                if day == today {
                    text = text.strong().color(ui.visuals().selection.bg_fill);
                }
                ui.add_sized(
                    [column_width, TIMELINE_HEADER_HEIGHT],
                    egui::Label::new(text),
                );
            }
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt(("calendar-timeline-scroll", self.block.id()))
            .max_height(TIMELINE_VIEW_HEIGHT)
            .show(ui, |ui| {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(available_width, TIMELINE_CONTENT_HEIGHT),
                    egui::Sense::click(),
                );
                let painter = ui.painter();
                for hour in 0..=24u32 {
                    let y = rect.top() + hour as f32 * HOUR_HEIGHT;
                    painter.line_segment(
                        [
                            egui::pos2(rect.left() + gutter, y),
                            egui::pos2(rect.right(), y),
                        ],
                        ui.visuals().widgets.noninteractive.bg_stroke,
                    );
                    if hour < 24 {
                        painter.text(
                            egui::pos2(rect.left() + 4.0, y + 2.0),
                            egui::Align2::LEFT_TOP,
                            format!("{hour:02}:00"),
                            egui::FontId::proportional(11.0),
                            ui.visuals().weak_text_color(),
                        );
                    }
                }
                for day_index in 0..=num_days {
                    let x = rect.left() + gutter + day_index as f32 * column_width;
                    painter.line_segment(
                        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                        ui.visuals().widgets.noninteractive.bg_stroke,
                    );
                }

                let mut event_rects = Vec::new();
                for day_index in 0..num_days {
                    let day = first_day + day_index;
                    let day_start_secs = day * SECONDS_PER_DAY;
                    let day_end_secs = day_start_secs + SECONDS_PER_DAY;
                    let mut day_events: Vec<&CalendarEvent> = events
                        .iter()
                        .filter(|event| event.start < day_end_secs && event.end > day_start_secs)
                        .collect();
                    day_events.sort_by_key(|event| event.start);
                    let (lanes, lane_count) = assign_lanes(&day_events);
                    let x = rect.left() + gutter + day_index as f32 * column_width;
                    for (event, lane) in day_events.iter().zip(lanes) {
                        let start_frac = ((event.start.max(day_start_secs) - day_start_secs)
                            as f32
                            / SECONDS_PER_DAY as f32)
                            .clamp(0.0, 1.0);
                        let end_frac = ((event.end.min(day_end_secs) - day_start_secs) as f32
                            / SECONDS_PER_DAY as f32)
                            .clamp(0.0, 1.0);
                        let top = rect.top() + start_frac * TIMELINE_CONTENT_HEIGHT;
                        let bottom = (rect.top() + end_frac * TIMELINE_CONTENT_HEIGHT)
                            .max(top + MIN_EVENT_HEIGHT);
                        let lane_width = (column_width - 2.0) / lane_count as f32;
                        let event_rect = egui::Rect::from_min_max(
                            egui::pos2(x + 1.0 + lane as f32 * lane_width, top + 1.0),
                            egui::pos2(
                                x + 1.0 + (lane + 1) as f32 * lane_width - 1.0,
                                bottom - 1.0,
                            ),
                        );
                        let id = ui.id().with(("calendar-timeline-event", event.id));
                        let event_response = ui.interact(event_rect, id, egui::Sense::click());
                        let fill = ui
                            .visuals()
                            .selection
                            .bg_fill
                            .gamma_multiply(if event_response.hovered() { 1.0 } else { 0.85 });
                        ui.painter().rect_filled(event_rect, 3.0, fill);
                        ui.painter().text(
                            event_rect.left_top() + egui::vec2(4.0, 2.0),
                            egui::Align2::LEFT_TOP,
                            truncated(&event.title, 24),
                            egui::FontId::proportional(12.0),
                            ui.visuals().strong_text_color(),
                        );
                        if event_response.clicked() {
                            self.form = Some(EventForm::edit(event));
                        }
                        event_rects.push(event_rect);
                    }
                }

                if response.clicked() {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        let inside_event = event_rects
                            .iter()
                            .any(|event_rect| event_rect.contains(pointer));
                        if !inside_event && pointer.x >= rect.left() + gutter {
                            let day_index = ((pointer.x - rect.left() - gutter) / column_width)
                                .floor()
                                .clamp(0.0, (num_days - 1) as f32)
                                as i64;
                            let hour = ((pointer.y - rect.top()) / HOUR_HEIGHT)
                                .floor()
                                .clamp(0.0, 23.0) as u8;
                            self.form = Some(EventForm::new_at(first_day + day_index, hour));
                        }
                    }
                }
            });
    }

    fn show_form(&mut self, ctx: &egui::Context) {
        let Some(form) = &mut self.form else {
            return;
        };
        let mut open = true;
        let mut save = false;
        let mut delete = false;
        let mut cancel = false;
        let title = if form.editing_id.is_some() {
            "Edit event"
        } else {
            "New event"
        };

        egui::Window::new(title)
            .id(egui::Id::new(("calendar-event-form", self.block.id())))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Title");
                    ui.text_edit_singleline(&mut form.title);
                });
                ui.label("Start");
                date_time_row(ui, &mut form.start);
                ui.label("End");
                date_time_row(ui, &mut form.end);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !form.title.trim().is_empty(),
                            egui::Button::new(format!("{} Save", ICON_SAVE.codepoint)),
                        )
                        .clicked()
                    {
                        save = true;
                    }
                    if form.editing_id.is_some()
                        && ui
                            .button(format!("{} Delete", ICON_DELETE.codepoint))
                            .clicked()
                    {
                        delete = true;
                    }
                    if ui
                        .button(format!("{} Cancel", ICON_CLOSE.codepoint))
                        .clicked()
                    {
                        cancel = true;
                    }
                });
            });

        if save {
            let form = self.form.take().expect("the form is shown above");
            let start = form.start.to_unix();
            let end = form.end.to_unix().max(start);
            let event = CalendarEvent {
                id: form.editing_id.unwrap_or_else(Uuid::new_v4),
                title: form.title.trim().to_owned(),
                start,
                end,
            };
            let operation = if form.editing_id.is_some() {
                CalendarOperation::UpdateEvent { event }
            } else {
                CalendarOperation::AddEvent { event }
            };
            self.block.operate(operation);
        } else if delete {
            let form = self.form.take().expect("the form is shown above");
            if let Some(id) = form.editing_id {
                self.block.operate(CalendarOperation::RemoveEvent { id });
            }
        } else if !open || cancel {
            self.form = None;
        }
    }
}

fn date_time_row(ui: &mut egui::Ui, fields: &mut DateTimeFields) {
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut fields.year).range(1..=9999));
        ui.add(egui::DragValue::new(&mut fields.month).range(1..=12));
        ui.label(MONTH_NAMES[(fields.month.clamp(1, 12) - 1) as usize]);
        let max_day = days_in_month(fields.year, fields.month.clamp(1, 12));
        fields.day = fields.day.clamp(1, max_day);
        ui.add(egui::DragValue::new(&mut fields.day).range(1..=max_day));
        ui.label("at");
        ui.add(
            egui::DragValue::new(&mut fields.hour)
                .range(0..=23)
                .custom_formatter(|value, _| format!("{value:02.0}")),
        );
        ui.label(":");
        ui.add(
            egui::DragValue::new(&mut fields.minute)
                .range(0..=59)
                .custom_formatter(|value, _| format!("{value:02.0}")),
        );
    });
}

/// Greedily assigns each of `events` (sorted by start) to the first lane
/// whose previous event has already ended, so overlapping events are shown
/// side by side instead of stacked on top of each other.
fn assign_lanes(events: &[&CalendarEvent]) -> (Vec<usize>, usize) {
    let mut lane_ends: Vec<i64> = Vec::new();
    let mut lanes = Vec::with_capacity(events.len());
    for event in events {
        match lane_ends.iter().position(|&end| end <= event.start) {
            Some(lane) => {
                lane_ends[lane] = event.end;
                lanes.push(lane);
            }
            None => {
                lanes.push(lane_ends.len());
                lane_ends.push(event.end);
            }
        }
    }
    (lanes, lane_ends.len().max(1))
}

fn truncated(text: &str, max_chars: usize) -> String {
    if text.chars().count() > max_chars {
        let mut result: String = text.chars().take(max_chars.saturating_sub(3)).collect();
        result.push_str("...");
        result
    } else {
        text.to_owned()
    }
}

fn week_start(day: i64) -> i64 {
    day - weekday_from_days(day) as i64
}

fn today_days_since_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| (duration.as_secs() / SECONDS_PER_DAY as u64) as i64)
        .unwrap_or(0)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 30,
    }
}

fn weekday_from_days(days_since_epoch: i64) -> u32 {
    (days_since_epoch + 4).rem_euclid(7) as u32
}

/// Howard Hinnant's civil-calendar algorithm: proleptic Gregorian
/// year/month/day to days since the Unix epoch.
fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = month as i32;
    let day = day as i32;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    (era * 146_097 + day_of_era - 719_468) as i64
}

/// The inverse of `days_from_civil`.
fn civil_from_days(days_since_epoch: i64) -> (i32, u8, u8) {
    let days = days_since_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era as i32 + era as i32 * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i32::from(month <= 2);
    (year, month as u8, day as u8)
}

impl BlockEditor for CalendarEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        Some(egui::vec2(
            DIRECT_EDITOR_WIDTH,
            match self.view {
                CalendarView::Month => MONTH_GRID_HEIGHT,
                CalendarView::Week | CalendarView::Day => TIMELINE_VIEW_HEIGHT + 60.0,
            },
        ))
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.toolbar(ui);
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let Some(calendar) = self.block.read() else {
            ui.spinner();
            return None;
        };
        let mut events = calendar.events().to_vec();
        drop(calendar);
        events.sort_by_key(|event| event.start);

        match self.view {
            CalendarView::Month => self.month_view(ui, &events),
            CalendarView::Week => {
                let first_day = week_start(self.anchor_day);
                self.timeline_view(ui, &events, first_day, 7);
            }
            CalendarView::Day => self.timeline_view(ui, &events, self.anchor_day, 1),
        }

        self.show_form(ui.ctx());
        None
    }
}
