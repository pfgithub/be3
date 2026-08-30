use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_enforces_rust_layout() {
    let root = temporary_directory();
    let module = root.join("crates/widget/src/widget");
    fs::create_dir_all(&module).unwrap();
    fs::write(
        module.join("mod.rs"),
        "pub fn value() -> u8 { 1 } // remove\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    fn expected() -> u8 { 1 }\n\n    #[test]\n    fn value_is_one() {\n        assert_eq!(value(), expected());\n    }\n}\n",
    )
    .unwrap();

    fix(&root, false).unwrap();

    let production = fs::read_to_string(root.join("crates/widget/src/widget.rs")).unwrap();
    let aggregator = fs::read_to_string(root.join("crates/widget/src/widget/tests.rs")).unwrap();
    let test =
        fs::read_to_string(root.join("crates/widget/src/widget/tests/value_is_one.rs")).unwrap();
    assert!(!module.join("mod.rs").exists());
    assert!(production.contains("mod tests;"));
    assert!(!production.contains("remove"));
    assert!(aggregator.contains("fn expected()"));
    assert!(aggregator.contains("mod value_is_one;"));
    assert!(test.contains("fn value_is_one()"));
    assert!(fix(&root, true).is_ok());

    fs::remove_dir_all(root).unwrap();
}
