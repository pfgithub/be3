use super::*;

#[test]
fn replace_ranges_replaces_every_range() {
    let mut tester = EditorTester::new("one two one two one");

    tester.execute(EditorCommand::ReplaceRanges {
        ranges: &[0..3, 8..11],
        replacement: b"six",
    });

    tester.expect_content("six| two six two one");
}
