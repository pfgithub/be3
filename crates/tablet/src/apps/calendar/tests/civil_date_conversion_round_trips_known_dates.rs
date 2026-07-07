use super::super::*;

#[test]
fn civil_date_conversion_round_trips_known_dates() {
    for (year, month, day) in [
        (1970, 1_u8, 1_u8),
        (2026, 7_u8, 7_u8),
        (2000, 2_u8, 29_u8),
        (2100, 3_u8, 1_u8),
    ] {
        assert_eq!(
            civil_from_days(days_from_civil(year, month, day)),
            (year, month, day)
        );
    }
}
