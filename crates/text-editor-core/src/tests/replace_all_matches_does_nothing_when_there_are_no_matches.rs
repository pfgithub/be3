use super::*;

#[test]
fn replace_all_matches_does_nothing_when_there_are_no_matches() {
    let mut tester = EditorTester::new(b"wow wow wow");
    tester.set_cursor(tester.pos(0));
    tester.execute(EditorCommand::ReplaceAllMatches {
        text: "xyz",
        case_sensitive: true,
        replacement: b"cat",
    });
    tester.expect_content(b"|wow wow wow");
}
