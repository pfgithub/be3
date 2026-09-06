use super::*;

#[test]
fn atomic_units_are_single_cursor_units() {
    let mut tester = EditorTester::new("a <<atom>> b");
    tester.editor.config.inside_atomic_unit = inside_double_angle_span;
    tester.set_cursor(tester.pos(2));

    tester.execute(EditorCommand::MoveCursorLeftRight {
        mode: MoveMode::Move,
        direction: LRDirection::Right,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });

    tester.expect_content(b"a <<atom>>| b");
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    tester.expect_content(b"a | b");
}

fn inside_double_angle_span(bytes: &[u8], index: usize) -> bool {
    let mut start = None;
    let mut position = 0;
    while position + 1 < bytes.len() {
        match &bytes[position..position + 2] {
            b"<<" => start = Some(position),
            b">>" => {
                if let Some(start) = start.take() {
                    if start < index && index < position + 2 {
                        return true;
                    }
                }
            }
            _ => {
                position += 1;
                continue;
            }
        }
        position += 2;
    }
    false
}
