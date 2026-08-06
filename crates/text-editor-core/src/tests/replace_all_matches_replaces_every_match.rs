use super::*;

#[test]
fn replace_all_matches_replaces_every_match() {
    let mut tester = EditorTester::new(b"wow wow wow");
    tester.execute(EditorCommand::ReplaceAllMatches {
        text: "wow",
        case_sensitive: true,
        replacement: b"meow",
    });
    tester.expect_content(b"meow meow meow|");
}
