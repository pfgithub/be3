use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_removes_test_path_attributes() {
    let root = temporary_directory();
    let source = root.join("src");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("lib.rs"),
        "#[cfg(test)]\n#[path = \"odd.rs\"]\nmod tests;\n",
    )
    .unwrap();
    fs::write(
        source.join("odd.rs"),
        "use super::*;\n\n#[test]\nfn canonical_test_path_is_used() {}\n",
    )
    .unwrap();

    fix(&root, false).unwrap();

    let production = fs::read_to_string(source.join("lib.rs")).unwrap();
    let aggregator = fs::read_to_string(source.join("tests.rs")).unwrap();
    assert!(!source.join("odd.rs").exists());
    assert!(!production.contains("#[path"));
    assert!(production.contains("mod tests;"));
    assert!(aggregator.contains("mod canonical_test_path_is_used;"));
    assert!(source.join("tests/canonical_test_path_is_used.rs").exists());
    assert!(fix(&root, true).is_ok());

    fs::remove_dir_all(root).unwrap();
}
