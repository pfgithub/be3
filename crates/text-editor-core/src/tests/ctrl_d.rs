use super::*;

#[test]
fn ctrl_d() {
    let mut tester = EditorTester::new(b"example example wow example");

    tester.expect_content(b"example example wow example");
    tester.set_cursor(tester.pos(0));
    tester.expect_content(b"|example example wow example");
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Right,
        mode: MoveMode::Select,
        stop: CursorLeftRightStop::Word,
    });
    tester.expect_content(b"[example| example wow example");
    tester.execute(EditorCommand::DuplicateCursor(LRDirection::Right));
    tester.expect_content(b"[example| [example| wow example");
    tester.execute(EditorCommand::DuplicateCursor(LRDirection::Right));
    tester.expect_content(b"[example| [example| wow [example|");
    tester.execute(EditorCommand::DuplicateCursor(LRDirection::Right));
    tester.expect_content(b"[example| [example| wow [example|");
    tester.set_cursor(tester.pos(15));
    tester.expect_content(b"example example| wow example");
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Left,
        mode: MoveMode::Select,
        stop: CursorLeftRightStop::Word,
    });
    tester.expect_content(b"example |example] wow example");
    tester.execute(EditorCommand::DuplicateCursor(LRDirection::Right));
    tester.expect_content(b"example |example] wow [example|");
    tester.execute(EditorCommand::DuplicateCursor(LRDirection::Right));
    tester.expect_content(b"[example| |example] wow [example|");
    tester.execute(EditorCommand::DuplicateCursor(LRDirection::Right));
    tester.expect_content(b"[example| |example] wow [example|");
    tester.execute(EditorCommand::InsertText(b"blah"));
    tester.expect_content(b"blah| blah| wow blah|");
    tester.set_cursor(tester.pos(9));
    tester.expect_content(b"blah blah| wow blah");
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Left,
        mode: MoveMode::Select,
        stop: CursorLeftRightStop::Word,
    });
    tester.expect_content(b"blah |blah] wow blah");
    tester.execute(EditorCommand::DuplicateCursor(LRDirection::Left));
    tester.expect_content(b"[blah| |blah] wow blah");
}
