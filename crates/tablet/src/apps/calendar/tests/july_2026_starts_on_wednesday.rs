use super::super::*;

#[test]
fn july_2026_starts_on_wednesday() {
    assert_eq!(
        Month {
            year: 2026,
            month: 7
        }
        .first_weekday(),
        3
    );
}
