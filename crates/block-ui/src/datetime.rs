const SECONDS_PER_DAY: i64 = 86_400;
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

#[derive(Clone, Copy)]
pub struct DateTimeFields {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
}

impl DateTimeFields {
    pub fn from_unix(seconds: i64) -> Self {
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

    pub fn to_unix(self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * SECONDS_PER_DAY
            + self.hour as i64 * 3600
            + self.minute as i64 * 60
    }
}

pub fn format_datetime_utc(seconds: i64) -> String {
    let fields = DateTimeFields::from_unix(seconds);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        fields.year, fields.month, fields.day, fields.hour, fields.minute
    )
}

pub fn parse_datetime_utc(value: &str) -> Option<i64> {
    let (date, time) = value.trim().split_once(' ')?;
    let mut date = date.split('-');
    let year = date.next()?.parse::<i32>().ok()?;
    let month = date.next()?.parse::<u8>().ok()?;
    let day = date.next()?.parse::<u8>().ok()?;
    if date.next().is_some() {
        return None;
    }
    let mut time = time.split(':');
    let hour = time.next()?.parse::<u8>().ok()?;
    let minute = time.next()?.parse::<u8>().ok()?;
    if time.next().is_some()
        || !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=days_in_month(year, month)).contains(&day)
        || hour > 23
        || minute > 59
    {
        return None;
    }
    Some(
        DateTimeFields {
            year,
            month,
            day,
            hour,
            minute,
        }
        .to_unix(),
    )
}

pub fn datetime_editor(ui: &mut egui::Ui, seconds: &mut i64) -> bool {
    let mut fields = DateTimeFields::from_unix(*seconds);
    let changed = date_time_fields_row(ui, &mut fields);
    if changed {
        *seconds = fields.to_unix();
    }
    changed
}

pub fn date_time_fields_row(ui: &mut egui::Ui, fields: &mut DateTimeFields) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui
            .add(egui::DragValue::new(&mut fields.year).range(1..=9999))
            .changed();
        changed |= ui
            .add(egui::DragValue::new(&mut fields.month).range(1..=12))
            .changed();
        ui.label(MONTH_NAMES[(fields.month.clamp(1, 12) - 1) as usize]);
        let max_day = days_in_month(fields.year, fields.month.clamp(1, 12));
        fields.day = fields.day.clamp(1, max_day);
        changed |= ui
            .add(egui::DragValue::new(&mut fields.day).range(1..=max_day))
            .changed();
        ui.label("at");
        changed |= ui
            .add(
                egui::DragValue::new(&mut fields.hour)
                    .range(0..=23)
                    .custom_formatter(|value, _| format!("{value:02.0}")),
            )
            .changed();
        ui.label(":");
        changed |= ui
            .add(
                egui::DragValue::new(&mut fields.minute)
                    .range(0..=59)
                    .custom_formatter(|value, _| format!("{value:02.0}")),
            )
            .changed();
    });
    changed
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 30,
    }
}

pub fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = month as i32;
    let day = day as i32;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    (era * 146_097 + day_of_era - 719_468) as i64
}

pub fn civil_from_days(days_since_epoch: i64) -> (i32, u8, u8) {
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
