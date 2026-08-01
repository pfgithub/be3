use super::*;
use block_client::block_url;
use uuid::Uuid;

#[test]
fn block_urls_are_single_cursor_units() {
    let url = block_url(Uuid::from_u128(1));
    let mut tester = EditorTester::new(format!("a {url} b"));
    tester.execute(EditorCommand::SetCursorPosition(tester.pos(2)));

    tester.execute(EditorCommand::MoveCursorLeftRight {
        mode: MoveMode::Move,
        direction: LRDirection::Right,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });

    tester.expect_content(format!("a {url}| b"));
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    tester.expect_content(b"a | b");
}
