use super::*;

#[test]
fn raw_bytes() {
    let mut tester = EditorTester::new([0xff, b'a']);
    tester.set_cursor(tester.pos(1));
    tester.execute(EditorCommand::InsertText(&[0x80, 0xfe]));
    assert_eq!(
        tester.editor.document().read().unwrap().bytes(),
        &[0xff, 0x80, 0xfe, b'a']
    );
    tester.execute(EditorCommand::Undo);
    assert_eq!(
        tester.editor.document().read().unwrap().bytes(),
        &[0xff, b'a']
    );
    tester.execute(EditorCommand::Redo);
    assert_eq!(
        tester.editor.document().read().unwrap().bytes(),
        &[0xff, 0x80, 0xfe, b'a']
    );
}
