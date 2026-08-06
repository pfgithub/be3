use super::*;

#[test]
fn find_status_reports_current_match_index() {
    let mut tester = EditorTester::new(b"wow wow wow");
    assert_eq!(
        tester.editor.find_status("wow", true),
        FindStatus {
            total: 3,
            current: None
        }
    );
    tester.execute(EditorCommand::Find {
        text: "wow",
        case_sensitive: true,
        direction: FindDirection::Next,
    });
    assert_eq!(
        tester.editor.find_status("wow", true),
        FindStatus {
            total: 3,
            current: Some(0)
        }
    );
    tester.execute(EditorCommand::Find {
        text: "wow",
        case_sensitive: true,
        direction: FindDirection::Next,
    });
    assert_eq!(
        tester.editor.find_status("wow", true),
        FindStatus {
            total: 3,
            current: Some(1)
        }
    );
}
