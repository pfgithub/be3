use super::*;

#[test]
fn apply_implicit_name_leaves_a_manual_name_untouched() {
    let mut properties = BTreeMap::new();
    properties.insert(
        NAME,
        encode_name(&BlockName {
            manual: true,
            value: "Renamed by hand".to_owned(),
        }),
    );

    apply_implicit_name(&mut properties, Some("Content changed".to_owned()));

    assert_eq!(
        read_name(&properties),
        Some(BlockName {
            manual: true,
            value: "Renamed by hand".to_owned(),
        })
    );
}
