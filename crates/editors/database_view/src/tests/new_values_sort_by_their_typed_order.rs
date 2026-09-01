use super::*;

#[test]
fn new_values_sort_by_their_typed_order() {
    let labels = HashMap::new();
    assert_eq!(
        compare_database_values(
            &DatabaseValue::Boolean(false),
            &DatabaseValue::Boolean(true),
            &field(DatabaseFieldType::Boolean),
            &labels,
        ),
        Ordering::Less
    );
    assert_eq!(
        compare_database_values(
            &DatabaseValue::Color(DatabaseColor {
                red: 0,
                green: 255,
                blue: 0,
                alpha: 255,
            }),
            &DatabaseValue::Color(DatabaseColor {
                red: 1,
                green: 0,
                blue: 0,
                alpha: 0,
            }),
            &field(DatabaseFieldType::Color),
            &labels,
        ),
        Ordering::Less
    );
    assert_eq!(
        compare_database_values(
            &DatabaseValue::Datetime(-1),
            &DatabaseValue::Datetime(0),
            &field(DatabaseFieldType::Datetime),
            &labels,
        ),
        Ordering::Less
    );
}
