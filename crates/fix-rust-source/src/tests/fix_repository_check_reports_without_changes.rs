use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_check_reports_without_changes() {
    let root = temporary_directory();
    let module = root.join("src/widget");
    fs::create_dir_all(&module).unwrap();
    let source = "pub fn value() {} // remove\n";
    fs::write(module.join("mod.rs"), source).unwrap();

    let error = fix(&root, true).unwrap_err().to_string();

    assert!(error.contains("module file: src/widget/mod.rs"));
    assert!(error.contains("comments: src/widget/mod.rs"));
    assert_eq!(fs::read_to_string(module.join("mod.rs")).unwrap(), source);
    assert!(!root.join("src/widget.rs").exists());

    fs::remove_dir_all(root).unwrap();
}
