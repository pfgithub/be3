use super::*;

#[test]
fn block_values_sort_by_resolved_label_then_reference() {
    let first = BlockRef::Direct(Uuid::from_u128(1));
    let second = BlockRef::Direct(Uuid::from_u128(2));
    let labels = HashMap::from([
        (
            first,
            BlockLabel {
                block_type: Uuid::nil(),
                icon: None,
                name: "Zulu".into(),
                automatic: false,
            },
        ),
        (
            second,
            BlockLabel {
                block_type: Uuid::nil(),
                icon: None,
                name: "Alpha".into(),
                automatic: false,
            },
        ),
    ]);
    assert_eq!(
        compare_database_values(
            &DatabaseValue::Block(first),
            &DatabaseValue::Block(second),
            &field(DatabaseFieldType::Block),
            &labels,
        ),
        Ordering::Greater
    );

    let tied = labels
        .into_iter()
        .map(|(reference, mut label)| {
            label.name = "Same".into();
            (reference, label)
        })
        .collect();
    assert_eq!(
        compare_database_values(
            &DatabaseValue::Block(first),
            &DatabaseValue::Block(second),
            &field(DatabaseFieldType::Block),
            &tied,
        ),
        Ordering::Less
    );
}
