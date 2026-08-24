use crate::renderer::{Color, Rect, Scene, Vector};
use crate::text::TextEngine;
use std::time::{SystemTime, UNIX_EPOCH};

const STATUS_BAR_HEIGHT: f32 = 54.0;
const OUTER_MARGIN: f32 = 22.0;
const HEADER_TOP: f32 = STATUS_BAR_HEIGHT + 24.0;
const HEADER_TEXT_SIZE: u32 = 34;
const DAY_TEXT_SIZE: u32 = 18;
const BUTTON_SIZE: f32 = 48.0;
const GRID_TOP_GAP: f32 = 76.0;
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

pub(crate) struct CalendarApp {
    visible_month: Month,
}

impl CalendarApp {
    pub(crate) fn new() -> Self {
        Self {
            visible_month: Month::today(),
        }
    }

    pub(crate) fn click(&mut self, size: Vector<2, f32>, position: Vector<2, f32>) -> bool {
        if previous_button_rect(size).contains(position) {
            self.visible_month = self.visible_month.previous();
            return true;
        }
        if next_button_rect(size).contains(position) {
            self.visible_month = self.visible_month.next();
            return true;
        }
        false
    }

    pub(crate) fn draw(&self, scene: &mut Scene, text: &mut TextEngine, size: Vector<2, f32>) {
        let month = self.visible_month;
        let header_y = HEADER_TOP;
        let month_name = month.name();
        let shaped_month_name = text.shape_text(month_name, HEADER_TEXT_SIZE);

        text.draw_shaped_text(
            scene,
            Vector::new(OUTER_MARGIN, header_y),
            &shaped_month_name,
            Color::BLACK,
        );
        text.draw_with_size(
            scene,
            Vector::new(OUTER_MARGIN + shaped_month_name.width() + 16.0, header_y),
            &month.year.to_string(),
            Color::GRAY,
            HEADER_TEXT_SIZE,
        );

        draw_month_button(scene, text, previous_button_rect(size), "<");
        draw_month_button(scene, text, next_button_rect(size), ">");
        self.draw_grid(scene, text, size);
    }

    fn draw_grid(&self, scene: &mut Scene, text: &mut TextEngine, size: Vector<2, f32>) {
        let grid = calendar_grid_rect(size);
        let cell_width = grid.size[0] / 7.0;
        let cell_height = grid.size[1] / 6.0;

        for (index, label) in WEEKDAYS.into_iter().enumerate() {
            text.draw(
                scene,
                Vector::new(
                    grid.position[0] + index as f32 * cell_width + 8.0,
                    grid.position[1] - 28.0,
                ),
                label,
                Color::BLACK,
            );
        }

        scene.stroke_rect(grid, 2.0, Color::BLACK);
        for column in 1..7 {
            let x = grid.position[0] + column as f32 * cell_width;
            scene.push_rect(
                Rect::new(
                    Vector::new(x, grid.position[1]),
                    Vector::new(2.0, grid.size[1]),
                ),
                Color::BLACK,
            );
        }
        for row in 1..6 {
            let y = grid.position[1] + row as f32 * cell_height;
            scene.push_rect(
                Rect::new(
                    Vector::new(grid.position[0], y),
                    Vector::new(grid.size[0], 2.0),
                ),
                Color::BLACK,
            );
        }

        let today = Date::today();
        for cell in 0..42 {
            let column = cell % 7;
            let row = cell / 7;
            let date = self.visible_month.date_for_cell(cell);
            let is_today = date == today;
            let text_position = Vector::new(
                grid.position[0] + column as f32 * cell_width + 9.0,
                grid.position[1] + row as f32 * cell_height + 8.0,
            );

            if is_today {
                scene.push_rect(
                    Rect::new(
                        Vector::new(text_position[0] - 3.0, text_position[1] - 2.0),
                        Vector::new(32.0, 28.0),
                    ),
                    Color::BLACK,
                );
            }

            text.draw_with_size(
                scene,
                text_position,
                &date.day.to_string(),
                if is_today {
                    Color::WHITE
                } else if date.month == self.visible_month {
                    Color::BLACK
                } else {
                    Color::GRAY
                },
                DAY_TEXT_SIZE,
            );
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Month {
    year: i32,
    month: u8,
}

impl Month {
    fn today() -> Self {
        let days_since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| (duration.as_secs() / 86_400) as i64)
            .unwrap_or(0);
        Date::from_days_since_epoch(days_since_epoch).month
    }

    fn previous(self) -> Self {
        if self.month == 1 {
            Self {
                year: self.year - 1,
                month: 12,
            }
        } else {
            Self {
                year: self.year,
                month: self.month - 1,
            }
        }
    }

    fn next(self) -> Self {
        if self.month == 12 {
            Self {
                year: self.year + 1,
                month: 1,
            }
        } else {
            Self {
                year: self.year,
                month: self.month + 1,
            }
        }
    }

    fn name(self) -> &'static str {
        match self.month {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => "",
        }
    }

    fn days_in_month(self) -> u32 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if is_leap_year(self.year) => 29,
            2 => 28,
            _ => 0,
        }
    }

    fn first_weekday(self) -> u32 {
        weekday_from_days(days_from_civil(self.year, self.month, 1))
    }

    fn date_for_cell(self, cell: u32) -> Date {
        let first_weekday = self.first_weekday();
        let days_this_month = self.days_in_month();
        if cell < first_weekday {
            let month = self.previous();
            Date {
                month,
                day: month.days_in_month() - first_weekday + cell + 1,
            }
        } else if cell >= first_weekday + days_this_month {
            Date {
                month: self.next(),
                day: cell - first_weekday - days_this_month + 1,
            }
        } else {
            Date {
                month: self,
                day: cell - first_weekday + 1,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Date {
    month: Month,
    day: u32,
}

impl Date {
    fn today() -> Self {
        let days_since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| (duration.as_secs() / 86_400) as i64)
            .unwrap_or(0);
        Self::from_days_since_epoch(days_since_epoch)
    }

    fn from_days_since_epoch(days_since_epoch: i64) -> Self {
        let (year, month, day) = civil_from_days(days_since_epoch);
        Self {
            month: Month { year, month },
            day: day.into(),
        }
    }
}

fn draw_month_button(scene: &mut Scene, text: &mut TextEngine, rect: Rect, label: &str) {
    scene.stroke_rect(rect, 2.0, Color::BLACK);
    text.draw_with_size(
        scene,
        Vector::new(rect.position[0] + 16.0, rect.position[1] + 7.0),
        label,
        Color::BLACK,
        28,
    );
}

fn previous_button_rect(size: Vector<2, f32>) -> Rect {
    let right = size[0] - OUTER_MARGIN;
    Rect::new(
        Vector::new(right - BUTTON_SIZE * 2.0 - 12.0, HEADER_TOP - 4.0),
        Vector::new(BUTTON_SIZE, BUTTON_SIZE),
    )
}

fn next_button_rect(size: Vector<2, f32>) -> Rect {
    Rect::new(
        Vector::new(size[0] - OUTER_MARGIN - BUTTON_SIZE, HEADER_TOP - 4.0),
        Vector::new(BUTTON_SIZE, BUTTON_SIZE),
    )
}

fn calendar_grid_rect(size: Vector<2, f32>) -> Rect {
    let top = HEADER_TOP + GRID_TOP_GAP;
    Rect::new(
        Vector::new(OUTER_MARGIN, top),
        Vector::new(
            (size[0] - OUTER_MARGIN * 2.0).max(7.0),
            (size[1] - top - OUTER_MARGIN).max(6.0),
        ),
    )
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn weekday_from_days(days_since_epoch: i64) -> u32 {
    (days_since_epoch + 4).rem_euclid(7) as u32
}

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

#[cfg(test)]
mod tests;
