use super::*;

#[test]
fn apply_implicit_name_clears_the_property_once_it_returns_none() {
    let mut properties = BTreeMap::new();
    apply_implicit_name(&mut properties, Some("First line".to_owned()));
    assert!(read_name(&properties).is_some());

    apply_implicit_name(&mut properties, None);

    assert_eq!(read_name(&properties), None);
}
