use super::*;

#[test]
fn a_painting_is_the_same_however_the_atlas_was_packed() {
    let alone = painted(&["a label", "a label"]);
    let after_other_text = painted(&["xyz 90210 /tmp/qjkw", "a label"]);

    assert_eq!(alone, after_other_text);
}
