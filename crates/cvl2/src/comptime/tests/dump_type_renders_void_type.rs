use super::*;

#[test]
fn dump_type_renders_void_type() {
    let dump = dump_type(&void_type(), usize::MAX);

    assert!(dump.contains("void"));
}
