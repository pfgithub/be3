use super::*;

#[test]
fn replace_match_replaces_current_and_advances_to_next_match() {
    let mut tester = EditorTester::new(b"wow wow wow");
    tester.execute(EditorCommand::Find {
        text: "wow",
        case_sensitive: true,
        direction: FindDirection::Next,
    });
    tester.execute(EditorCommand::ReplaceMatch {
        text: "wow",
        case_sensitive: true,
        replacement: b"cat",
    });
    tester.expect_content(b"cat [wow| wow");
}
