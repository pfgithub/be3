use super::super::*;

#[test]
fn date_for_cell_includes_adjacent_months() {
    let july_2026 = Month {
        year: 2026,
        month: 7,
    };

    assert_eq!(
        july_2026.date_for_cell(0),
        Date {
            month: Month {
                year: 2026,
                month: 6
            },
            day: 28
        }
    );
    assert_eq!(
        july_2026.date_for_cell(3),
        Date {
            month: july_2026,
            day: 1
        }
    );
    assert_eq!(
        july_2026.date_for_cell(33),
        Date {
            month: july_2026,
            day: 31
        }
    );
    assert_eq!(
        july_2026.date_for_cell(34),
        Date {
            month: Month {
                year: 2026,
                month: 8
            },
            day: 1
        }
    );
}
