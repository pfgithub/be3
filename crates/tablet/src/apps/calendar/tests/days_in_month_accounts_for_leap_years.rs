use super::super::*;

#[test]
fn days_in_month_accounts_for_leap_years() {
    assert_eq!(
        Month {
            year: 2024,
            month: 2
        }
        .days_in_month(),
        29
    );
    assert_eq!(
        Month {
            year: 2026,
            month: 2
        }
        .days_in_month(),
        28
    );
    assert_eq!(
        Month {
            year: 2100,
            month: 2
        }
        .days_in_month(),
        28
    );
    assert_eq!(
        Month {
            year: 2000,
            month: 2
        }
        .days_in_month(),
        29
    );
}
