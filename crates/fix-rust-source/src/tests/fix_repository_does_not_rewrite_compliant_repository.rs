use super::*;
use crate::fix_repository as fix;
use std::fs;
use std::time::SystemTime;

#[test]
fn fix_repository_does_not_rewrite_compliant_repository() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(source.join("tests")).unwrap();
    fs::write(source.join("lib.rs"), "#[cfg(test)]\nmod tests;\n").unwrap();
    fs::write(source.join("tests.rs"), "use super::*;\n\nmod value;\n").unwrap();
    fs::write(
        source.join("tests/value.rs"),
        "use super::*;\n\n#[test]\nfn value() {}\n",
    )
    .unwrap();
    let paths = [
        source.join("lib.rs"),
        source.join("tests.rs"),
        source.join("tests/value.rs"),
    ];
    let times = fs::FileTimes::new().set_modified(SystemTime::UNIX_EPOCH);
    for path in &paths {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(times)
            .unwrap();
    }

    fix(&root, false).unwrap();

    for path in paths {
        assert_eq!(
            fs::metadata(path).unwrap().modified().unwrap(),
            SystemTime::UNIX_EPOCH
        );
    }
    fs::remove_dir_all(root).unwrap();
}
