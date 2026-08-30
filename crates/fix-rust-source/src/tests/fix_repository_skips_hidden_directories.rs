use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_skips_hidden_directories() {
    let root = temporary_directory();
    let cache = root.join(".zig-cache/package/src");
    fs::create_dir_all(&cache).unwrap();
    let source = "pub fn value() {} // remove\n";
    fs::write(cache.join("main.rs"), source).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn value() {}\n").unwrap();

    fix(&root, true).unwrap();

    assert_eq!(fs::read_to_string(cache.join("main.rs")).unwrap(), source);

    fs::remove_dir_all(root).unwrap();
}
