use crate::strip_comments as strip;

#[test]
fn strip_comments() {
    let source = br##"fn main() {
    let url = "https://example.com/a/*b*/"; // removed
    let raw = r#"// retained"#;
    let value = 1 /* removed
    across lines */ + 2;
}
"##;

    let stripped = strip(source).unwrap();
    let stripped = String::from_utf8(stripped).unwrap();

    assert!(stripped.contains("https://example.com/a/*b*/"));
    assert!(stripped.contains(r##"r#"// retained"#"##));
    assert!(!stripped.contains("removed"));
    assert_eq!(
        stripped.lines().count(),
        String::from_utf8_lossy(source).lines().count()
    );
    assert!(stripped.contains("let value = 1\n + 2;"));
}
