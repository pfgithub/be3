use super::*;

#[test]
fn read_name_returns_none_without_the_property() {
    assert_eq!(read_name(&BTreeMap::new()), None);
}
