use super::super::parse_markdown_checkboxes;

#[test]
fn parses_markdown_checkboxes() {
    let checkboxes = parse_markdown_checkboxes(b"- [ ] first\n  * [x] second\n- ordinary");

    assert_eq!(checkboxes.len(), 2);
    assert_eq!(checkboxes[0].marker, 0..5);
    assert!(!checkboxes[0].checked);
    assert_eq!(checkboxes[1].marker, 14..19);
    assert!(checkboxes[1].checked);
}
