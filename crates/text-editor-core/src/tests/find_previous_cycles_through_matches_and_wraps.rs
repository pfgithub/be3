use super::*;

#[test]
fn find_previous_cycles_through_matches_and_wraps() {
    let mut tester = EditorTester::new(b"wow wow wow");
    tester.execute(EditorCommand::Find {
        text: "wow",
        case_sensitive: true,
        direction: FindDirection::Previous,
    });
    tester.expect_content(b"wow wow [wow|");
    tester.execute(EditorCommand::Find {
        text: "wow",
        case_sensitive: true,
        direction: FindDirection::Previous,
    });
    tester.expect_content(b"wow [wow| wow");
    tester.execute(EditorCommand::Find {
        text: "wow",
        case_sensitive: true,
        direction: FindDirection::Previous,
    });
    tester.expect_content(b"[wow| wow wow");
    tester.execute(EditorCommand::Find {
        text: "wow",
        case_sensitive: true,
        direction: FindDirection::Previous,
    });
    tester.expect_content(b"wow wow [wow|");
}
