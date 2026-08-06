use super::*;

#[test]
fn replace_match_does_nothing_when_selection_is_not_on_a_match() {
    let mut tester = EditorTester::new(b"wow wow wow");
    tester.set_cursor(tester.pos(0));
    tester.execute(EditorCommand::ReplaceMatch {
        text: "wow",
        case_sensitive: true,
        replacement: b"cat",
    });
    tester.expect_content(b"|wow wow wow");
}
