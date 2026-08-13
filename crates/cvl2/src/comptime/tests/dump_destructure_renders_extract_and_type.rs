use super::*;

#[test]
fn dump_destructure_renders_extract_and_type() {
    let destructure = Destructure {
        extract: DestructureExtract::SingleItem {
            name: "a".to_string(),
            pos: pos_at(0),
        },
        ty: void_type(),
    };

    let dump = dump_destructure(&destructure, usize::MAX);

    assert!(dump.contains("extract="));
    assert!(dump.contains("single_item"));
    assert!(dump.contains("type="));
    assert!(dump.contains("void"));
}
