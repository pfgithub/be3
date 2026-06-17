use super::*;

#[test]
fn validates_component_names() {
    for valid in ["Adder", "half adder", "mux_2-1", "A1"] {
        assert_eq!(validate_name(valid).unwrap(), valid);
    }
    for invalid in ["", " leading", "trailing ", "../escape", "emoji!"] {
        assert!(validate_name(invalid).is_err(), "{invalid:?}");
    }
    assert!(validate_name(&"a".repeat(65)).is_err());
}
