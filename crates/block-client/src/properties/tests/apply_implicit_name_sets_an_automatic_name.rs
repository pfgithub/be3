use super::*;

#[test]
fn apply_implicit_name_sets_an_automatic_name() {
    let mut properties = BTreeMap::new();
    apply_implicit_name(&mut properties, Some("First line".to_owned()));
    assert_eq!(
        read_name(&properties),
        Some(BlockName {
            manual: false,
            value: "First line".to_owned(),
        })
    );

    apply_implicit_name(&mut properties, Some("Updated line".to_owned()));
    assert_eq!(
        read_name(&properties),
        Some(BlockName {
            manual: false,
            value: "Updated line".to_owned(),
        })
    );
}
