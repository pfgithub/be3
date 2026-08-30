use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_ignores_sources_outside_crates() {
    let root = temporary_directory();
    let outside = root.join("examples/widget/src");
    fs::create_dir_all(outside.join("module")).unwrap();
    let source = "pub fn value() {} // remove\n";
    fs::write(outside.join("module/mod.rs"), source).unwrap();
    fs::create_dir_all(root.join("crates/widget/src")).unwrap();
    fs::write(root.join("crates/widget/src/lib.rs"), "pub fn value() {}\n").unwrap();

    fix(&root, true).unwrap();
    fix(&root, false).unwrap();

    assert_eq!(
        fs::read_to_string(outside.join("module/mod.rs")).unwrap(),
        source
    );

    fs::remove_dir_all(root).unwrap();
}
