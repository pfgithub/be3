use super::super::*;

#[test]
fn month_navigation_wraps_across_years() {
    assert_eq!(
        Month {
            year: 2026,
            month: 1
        }
        .previous(),
        Month {
            year: 2025,
            month: 12
        }
    );
    assert_eq!(
        Month {
            year: 2026,
            month: 12
        }
        .next(),
        Month {
            year: 2027,
            month: 1
        }
    );
}
