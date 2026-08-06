use super::*;

#[test]
fn core() {
    let mut tester = EditorTester::new(b"hello!");

    tester.expect_content(b"hello!");
    tester.set_cursor(tester.pos(0));
    tester.expect_content(b"|hello!");
    tester.execute(EditorCommand::InsertText(b"abcd!"));
    tester.expect_content(b"abcd!|hello!");
    tester.set_cursor(tester.pos(0));
    tester.expect_content(b"|abcd!hello!");
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Right,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"|bcd!hello!");
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"|bcd!hello!");
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Right,
        stop: CursorLeftRightStop::Byte,
        mode: MoveMode::Select,
    });
    tester.expect_content(b"[b|cd!hello!");
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Right,
        stop: CursorLeftRightStop::Byte,
        mode: MoveMode::Select,
    });
    tester.expect_content(b"[bc|d!hello!");
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Right,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"|d!hello!");
    tester.execute(EditorCommand::InsertText("……".as_bytes()));
    assert_eq!(tester.editor.cursor_positions().len(), 1);
    tester.expect_content("……|d!hello!".as_bytes());
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::Codepoint,
    });
    tester.expect_content("…|d!hello!".as_bytes());
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Right,
        stop: CursorLeftRightStop::Line,
    });
    tester.expect_content("…|".as_bytes());
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::Line,
    });
    tester.expect_content(b"|");
    tester.execute(EditorCommand::InsertText(b"    hi();"));
    tester.expect_content(b"    hi();|");
    tester.execute(EditorCommand::Newline);
    tester.expect_content(b"    hi();\n    |");
    tester.execute(EditorCommand::InsertText(b"goodbye();"));
    tester.expect_content(b"    hi();\n    goodbye();|");
    tester.execute(EditorCommand::IndentSelection(LRDirection::Left));
    tester.expect_content(b"    hi();\ngoodbye();|");
    tester.execute(EditorCommand::IndentSelection(LRDirection::Right));
    tester.expect_content(b"    hi();\n    goodbye();|");
    tester.execute(EditorCommand::IndentSelection(LRDirection::Right));
    tester.expect_content(b"    hi();\n        goodbye();|");
    tester.execute(EditorCommand::Newline);
    tester.expect_content(b"    hi();\n        goodbye();\n        |");
    tester.execute(EditorCommand::IndentSelection(LRDirection::Left));
    tester.execute(EditorCommand::IndentSelection(LRDirection::Left));
    tester.expect_content(b"    hi();\n        goodbye();\n|");
    tester.execute(EditorCommand::IndentSelection(LRDirection::Right));
    tester.expect_content(b"    hi();\n        goodbye();\n    |");
    tester.execute(EditorCommand::SelectAll);
    tester.expect_content(b"[    hi();\n        goodbye();\n    |");
    tester.execute(EditorCommand::IndentSelection(LRDirection::Left));
    tester.expect_content(b"[hi();\n    goodbye();\n|");
    tester.execute(EditorCommand::IndentSelection(LRDirection::Right));
    tester.expect_content(b"    [hi();\n        goodbye();\n|");
    tester.execute(EditorCommand::IndentSelection(LRDirection::Left));
    tester.expect_content(b"[hi();\n    goodbye();\n|");
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Right,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"|");
    tester.execute(EditorCommand::InsertText(b"hello\nto the world!"));
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::Byte,
        mode: MoveMode::Move,
    });
    tester.expect_content(b"hello\nto the world|!");
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Up,
        metric: CursorHorizontalPositionMetric::Byte,
        mode: VerticalMoveMode::Move,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"hello|\nto the world!");
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Down,
        metric: CursorHorizontalPositionMetric::Byte,
        mode: VerticalMoveMode::Move,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"hello\nto the world|!");
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Up,
        metric: CursorHorizontalPositionMetric::Byte,
        mode: VerticalMoveMode::Move,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"hello|\nto the world!");
    tester.execute(EditorCommand::InsertText(b"!"));
    tester.expect_content(b"hello!|\nto the world!");
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Down,
        metric: CursorHorizontalPositionMetric::Byte,
        mode: VerticalMoveMode::Move,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"hello!\nto the| world!");

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(b"hello"));
    tester.expect_content(b"hello|");
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Left,
        mode: MoveMode::Move,
        stop: CursorLeftRightStop::Byte,
    });
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Left,
        mode: MoveMode::Move,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"hel|lo");
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Down,
        metric: CursorHorizontalPositionMetric::Byte,
        mode: VerticalMoveMode::Select,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"hel[lo|");

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(b"hela\n\ninput\n\n\nlo!"));
    tester.expect_content(b"hela\n\ninput\n\n\nlo!|");
    for expected in [
        b"hela\n\ninput\n\n|\nlo!".as_slice(),
        b"hela\n\ninput\n|\n\nlo!".as_slice(),
        b"hela\n\ninp|ut\n\n\nlo!".as_slice(),
    ] {
        tester.execute(EditorCommand::MoveCursorUpDown {
            direction: UDDirection::Up,
            metric: CursorHorizontalPositionMetric::Byte,
            mode: VerticalMoveMode::Move,
            stop: CursorLeftRightStop::Byte,
        });
        tester.expect_content(expected);
    }
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Right,
        mode: MoveMode::Move,
        stop: CursorLeftRightStop::Byte,
    });
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Right,
        mode: MoveMode::Move,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"hela\n\ninput|\n\n\nlo!");
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Up,
        metric: CursorHorizontalPositionMetric::Byte,
        mode: VerticalMoveMode::Move,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"hela\n|\ninput\n\n\nlo!");
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Up,
        metric: CursorHorizontalPositionMetric::Byte,
        mode: VerticalMoveMode::Move,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"hela|\n\ninput\n\n\nlo!");

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(
        b"here are a few words to traverse!",
    ));
    tester.expect_content(b"here are a few words to traverse!|");
    for expected in [
        b"here are a few words to traverse|!".as_slice(),
        b"here are a few words to |traverse!".as_slice(),
        b"here are a few words |to traverse!".as_slice(),
        b"here are a few |words to traverse!".as_slice(),
        b"here are a |few words to traverse!".as_slice(),
        b"here are |a few words to traverse!".as_slice(),
        b"here |are a few words to traverse!".as_slice(),
        b"|here are a few words to traverse!".as_slice(),
        b"|here are a few words to traverse!".as_slice(),
    ] {
        tester.execute(EditorCommand::MoveCursorLeftRight {
            direction: LRDirection::Left,
            mode: MoveMode::Move,
            stop: CursorLeftRightStop::Word,
        });
        tester.expect_content(expected);
    }
    for expected in [
        b"here| are a few words to traverse!".as_slice(),
        b"here are| a few words to traverse!".as_slice(),
        b"here are a| few words to traverse!".as_slice(),
        b"here are a few| words to traverse!".as_slice(),
        b"here are a few words| to traverse!".as_slice(),
        b"here are a few words to| traverse!".as_slice(),
        b"here are a few words to traverse|!".as_slice(),
        b"here are a few words to traverse!|".as_slice(),
        b"here are a few words to traverse!|".as_slice(),
    ] {
        tester.execute(EditorCommand::MoveCursorLeftRight {
            direction: LRDirection::Right,
            mode: MoveMode::Move,
            stop: CursorLeftRightStop::Word,
        });
        tester.expect_content(expected);
    }
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Up,
        metric: CursorHorizontalPositionMetric::Byte,
        mode: VerticalMoveMode::Move,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"|here are a few words to traverse!");
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Down,
        metric: CursorHorizontalPositionMetric::Byte,
        mode: VerticalMoveMode::Move,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"here are a few words to traverse!|");

    tester.execute(EditorCommand::Click {
        position: tester.pos(13),
        mode: DragSelectionMode::default(),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"here are a fe|w words to traverse!");
    tester.execute(EditorCommand::Click {
        position: tester.pos(17),
        mode: DragSelectionMode::default(),
        extend: true,
        select_syntax_node: false,
    });
    tester.expect_content(b"here are a fe[w wo|rds to traverse!");
    tester.execute(EditorCommand::Click {
        position: tester.pos(6),
        mode: DragSelectionMode::default(),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"here a|re a few words to traverse!");
    tester.execute(EditorCommand::Click {
        position: tester.pos(6),
        mode: DragSelectionMode::select(CursorLeftRightStop::Word),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"here [are| a few words to traverse!");
    tester.execute(EditorCommand::Drag(tester.pos(13)));
    tester.expect_content(b"here [are a few| words to traverse!");
    tester.execute(EditorCommand::Drag(tester.pos(1)));
    tester.expect_content(b"|here are] a few words to traverse!");

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(
        b"    \\\\    }\n    \\\\    @vertex fn vert(in: VertexIn)",
    ));
    tester.set_cursor(tester.pos(11));
    tester.expect_content(b"    \\\\    }|\n    \\\\    @vertex fn vert(in: VertexIn)");
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Right,
        stop: CursorLeftRightStop::Word,
        mode: MoveMode::Move,
    });
    tester.expect_content(b"    \\\\    }\n    \\\\|    @vertex fn vert(in: VertexIn)");

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(
        "He\u{301}! …मनीष!👨‍👩‍👧‍👧/🇷🇸🇮🇴/!\r\n!\n.".as_bytes(),
    ));
    tester.expect_content("He\u{301}! …मनीष!👨‍👩‍👧‍👧/🇷🇸🇮🇴/!\r\n!\n.|".as_bytes());
    for expected in [
        "He\u{301}! …मनीष!👨‍👩‍👧‍👧/🇷🇸🇮🇴/!\r\n!\n|",
        "He\u{301}! …मनीष!👨‍👩‍👧‍👧/🇷🇸🇮🇴/!\r\n!|",
        "He\u{301}! …मनीष!👨‍👩‍👧‍👧/🇷🇸🇮🇴/!\r\n|",
        "He\u{301}! …मनीष!👨‍👩‍👧‍👧/🇷🇸🇮🇴/!|",
        "He\u{301}! …मनीष!👨‍👩‍👧‍👧/🇷🇸🇮🇴/|",
        "He\u{301}! …मनीष!👨‍👩‍👧‍👧/🇷🇸🇮🇴|",
        "He\u{301}! …मनीष!👨‍👩‍👧‍👧/🇷🇸|",
        "He\u{301}! …मनीष!👨‍👩‍👧‍👧/|",
        "He\u{301}! …मनीष!👨‍👩‍👧‍👧|",
        "He\u{301}! …मनीष!|",
        "He\u{301}! …मनीष|",
        "He\u{301}! …मनी|",
        "He\u{301}! …म|",
        "He\u{301}! …|",
        "He\u{301}! |",
        "He\u{301}!|",
        "He\u{301}|",
        "H|",
        "|",
    ] {
        tester.execute(EditorCommand::Delete {
            direction: LRDirection::Left,
            stop: CursorLeftRightStop::UnicodeGraphemeCluster,
        });
        tester.expect_content(expected.as_bytes());
    }

    tester.execute(EditorCommand::InsertText("e\u{301}".as_bytes()));
    tester.expect_content("e\u{301}|".as_bytes());
    tester.execute(EditorCommand::Click {
        position: tester.pos(1),
        mode: DragSelectionMode::default(),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content("|e\u{301}".as_bytes());
    for (position, expected) in [
        (2, "|e\u{301}"),
        (3, "[e\u{301}|"),
        (2, "|e\u{301}"),
        (1, "|e\u{301}"),
        (0, "|e\u{301}"),
    ] {
        tester.execute(EditorCommand::Drag(tester.pos(position)));
        tester.expect_content(expected.as_bytes());
    }

    core_second_half(&mut tester);
}

fn core_second_half(tester: &mut EditorTester) {
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(b"line 1\nline 2\nline 3\nline 4"));
    tester.expect_content(b"line 1\nline 2\nline 3\nline 4|");
    for expected in [
        b"line 1\nline 2\nline 3|\nline 4|".as_slice(),
        b"line 1\nline 2|\nline 3|\nline 4|".as_slice(),
        b"line 1|\nline 2|\nline 3|\nline 4|".as_slice(),
    ] {
        tester.execute(EditorCommand::MoveCursorUpDown {
            direction: UDDirection::Up,
            metric: CursorHorizontalPositionMetric::Byte,
            mode: VerticalMoveMode::Duplicate,
            stop: CursorLeftRightStop::Byte,
        });
        tester.expect_content(expected);
    }
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"line |\nline |\nline |\nline |");
    tester.execute(EditorCommand::InsertText(b"5"));
    tester.expect_content(b"line 5|\nline 5|\nline 5|\nline 5|");

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Right,
        stop: CursorLeftRightStop::Word,
        mode: MoveMode::Move,
    });
    tester.expect_content(b"line 5\nline 5\nline 5\nline 5|");
    tester.execute(EditorCommand::DuplicateLine(UDDirection::Up));
    tester.expect_content(b"line 5\nline 5\nline 5\nline 5|\nline 5");
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::Byte,
    });
    tester.execute(EditorCommand::InsertText(b"9"));
    tester.expect_content(b"line 5\nline 5\nline 5\nline 9|\nline 5");
    tester.execute(EditorCommand::DuplicateLine(UDDirection::Down));
    tester.expect_content(b"line 5\nline 5\nline 5\nline 9\nline 9|\nline 5");

    syntax_node_and_tail(tester);
}

fn syntax_node_and_tail(tester: &mut EditorTester) {
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(
        b"pub fn demo() !u8 {\n    return 5;\n}\n",
    ));
    tester.execute(EditorCommand::Click {
        position: tester.pos(29),
        mode: DragSelectionMode::default(),
        extend: false,
        select_syntax_node: true,
    });
    tester.expect_content(b"pub fn demo() !u8 {\n    [return| 5;\n}\n");
    for (position, expected) in [
        (29, b"pub fn demo() !u8 {\n    [return| 5;\n}\n".as_slice()),
        (27, b"pub fn demo() !u8 {\n    |return] 5;\n}\n".as_slice()),
        (23, b"pub fn demo() !u8 |{\n    return 5;\n}]\n".as_slice()),
        (8, b"pub |fn demo() !u8 {\n    return 5;\n}]\n".as_slice()),
    ] {
        tester.execute(EditorCommand::Drag(tester.pos(position)));
        tester.expect_content(expected);
    }
    tester.execute(EditorCommand::Click {
        position: tester.pos(29),
        mode: DragSelectionMode::default(),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"pub fn demo() !u8 {\n    retur|n 5;\n}\n");
    for expected in [
        b"pub fn demo() !u8 {\n    [return| 5;\n}\n".as_slice(),
        b"pub fn demo() !u8 {\n    [return 5|;\n}\n".as_slice(),
        b"pub fn demo() !u8 {\n    [return 5;|\n}\n".as_slice(),
        b"pub fn demo() !u8 [{\n    return 5;\n}|\n".as_slice(),
        b"pub [fn demo() !u8 {\n    return 5;\n}|\n".as_slice(),
        b"[pub fn demo() !u8 {\n    return 5;\n}\n|".as_slice(),
        b"[pub fn demo() !u8 {\n    return 5;\n}\n|".as_slice(),
    ] {
        tester.execute(EditorCommand::SelectSyntaxNode(SyntaxNodeDirection::Parent));
        tester.expect_content(expected);
    }
    for expected in [
        b"pub [fn demo() !u8 {\n    return 5;\n}|\n".as_slice(),
        b"pub fn demo() !u8 [{\n    return 5;\n}|\n".as_slice(),
        b"pub fn demo() !u8 {\n    [return 5;|\n}\n".as_slice(),
        b"pub fn demo() !u8 {\n    [return 5|;\n}\n".as_slice(),
        b"pub fn demo() !u8 {\n    [return| 5;\n}\n".as_slice(),
        b"pub fn demo() !u8 {\n    retur|n 5;\n}\n".as_slice(),
        b"pub fn demo() !u8 {\n    retur|n 5;\n}\n".as_slice(),
    ] {
        tester.execute(EditorCommand::SelectSyntaxNode(SyntaxNodeDirection::Child));
        tester.expect_content(expected);
    }
    tester.execute(EditorCommand::SelectSyntaxNode(SyntaxNodeDirection::Child));

    tester.execute(EditorCommand::InsertLine(UDDirection::Down));
    tester.expect_content(b"pub fn demo() !u8 {\n    return 5;\n    |\n}\n");
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::Line,
    });
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    tester.expect_content(b"pub fn demo() !u8 {\n    return 5;|\n}\n");
    tester.execute(EditorCommand::InsertLine(UDDirection::Up));
    tester.expect_content(b"pub fn demo() !u8 {\n    |\n    return 5;\n}\n");

    copy_paste_undo_tail(tester);
}

fn copy_paste_undo_tail(tester: &mut EditorTester) {
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Down,
        mode: VerticalMoveMode::Duplicate,
        metric: CursorHorizontalPositionMetric::Byte,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"pub fn demo() !u8 {\n    |\n    |return 5;\n}\n");
    let copied = tester.editor.copy_utf8(CopyMode::Cut);
    tester.expect_content(b"pub fn demo() !u8 {\n|}\n");
    tester.execute(EditorCommand::Paste(copied.as_bytes()));
    tester.expect_content(b"pub fn demo() !u8 {\n    \n    return 5;\n|}\n");

    tester.execute(EditorCommand::Click {
        position: tester.pos(4),
        mode: DragSelectionMode::default(),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"pub |fn demo() !u8 {\n    \n    return 5;\n}\n");
    let copied = tester.editor.copy_utf8(CopyMode::Cut);
    tester.expect_content(b"|    \n    return 5;\n}\n");
    tester.execute(EditorCommand::Click {
        position: tester.pos(10),
        mode: DragSelectionMode::default(),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"    \n    r|eturn 5;\n}\n");
    tester.execute(EditorCommand::Paste(copied.as_bytes()));
    tester.expect_content(b"    \npub fn demo() !u8 {\n    r|eturn 5;\n}\n");
    tester.execute(EditorCommand::Click {
        position: tester.pos(6),
        mode: DragSelectionMode::default(),
        extend: false,
        select_syntax_node: false,
    });
    tester.execute(EditorCommand::Click {
        position: tester.pos(6),
        mode: DragSelectionMode::select(CursorLeftRightStop::Word),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"    \n[pub| fn demo() !u8 {\n    return 5;\n}\n");
    let copied = tester.editor.copy_utf8(CopyMode::Cut);
    tester.expect_content(b"    \n| fn demo() !u8 {\n    return 5;\n}\n");
    tester.execute(EditorCommand::Click {
        position: tester.pos(14),
        mode: DragSelectionMode::default(),
        extend: false,
        select_syntax_node: false,
    });
    tester.execute(EditorCommand::Paste(copied.as_bytes()));
    tester.expect_content(b"    \n fn demo(pub|) !u8 {\n    return 5;\n}\n");
    tester.execute(EditorCommand::Paste(b", const"));
    tester.expect_content(b"    \n fn demo(pub, const|) !u8 {\n    return 5;\n}\n");

    let copied = tester.editor.copy_utf8(CopyMode::Cut);
    tester.expect_content(b"    \n|    return 5;\n}\n");
    tester.execute(EditorCommand::Paste(b"abc"));
    tester.expect_content(b"    \nabc|    return 5;\n}\n");
    tester.execute(EditorCommand::Paste(copied.as_bytes()));
    tester.expect_content(b"    \nabc fn demo(pub, const) !u8 {\n|    return 5;\n}\n");

    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::Byte,
        mode: MoveMode::Select,
    });
    tester.expect_content(b"    \nabc fn demo(pub, const) !u8 {|\n]    return 5;\n}\n");
    tester.execute(EditorCommand::InsertLine(UDDirection::Down));
    tester.expect_content(b"    \nabc fn demo(pub, const) !u8 {\n|\n    return 5;\n}\n");

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"|");
    for expected in [
        b"\n|".as_slice(),
        b"\n\n|".as_slice(),
        b"\n\n\n|".as_slice(),
    ] {
        tester.execute(EditorCommand::InsertLine(UDDirection::Down));
        tester.expect_content(expected);
    }
    for expected in [b"\n\n|".as_slice(), b"\n|".as_slice(), b"|".as_slice()] {
        tester.execute(EditorCommand::Undo);
        tester.expect_content(expected);
    }
    for expected in [
        b"\n|".as_slice(),
        b"\n\n|".as_slice(),
        b"\n\n\n|".as_slice(),
    ] {
        tester.execute(EditorCommand::Redo);
        tester.expect_content(expected);
    }
    tester.execute(EditorCommand::Redo);
    tester.expect_content(b"\n\n\n|");
    tester.execute(EditorCommand::Undo);
    tester.expect_content(b"\n\n|");
    tester.execute(EditorCommand::SelectAll);
    tester.expect_content(b"[\n\n|");
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::Byte,
    });
    tester.expect_content(b"|");
    tester.execute(EditorCommand::Undo);
    tester.expect_content(b"[\n\n|");
    tester.execute(EditorCommand::Redo);
    tester.expect_content(b"|");

    for byte in b"const std = @import(\"std\");" {
        tester.execute(EditorCommand::InsertText(std::slice::from_ref(byte)));
    }
    tester.expect_content(b"const std = @import(\"std\");|");
    for expected in [
        b"const std = @import(|".as_slice(),
        b"const std = @import|".as_slice(),
        b"const std =|".as_slice(),
        b"const std|".as_slice(),
        b"const|".as_slice(),
        b"|".as_slice(),
    ] {
        tester.execute(EditorCommand::Undo);
        tester.expect_content(expected);
    }
    for _ in 0..6 {
        tester.execute(EditorCommand::Redo);
    }
    tester.expect_content(b"const std = @import(\"std\");|");
    let length = tester.editor.document().read().unwrap().len();
    for _ in 0..length {
        tester.execute(EditorCommand::Delete {
            direction: LRDirection::Left,
            stop: CursorLeftRightStop::Byte,
        });
    }
    tester.expect_content(b"|");
    tester.execute(EditorCommand::Undo);
    tester.expect_content(b"const std = @import(\"std\");|");
    tester.execute(EditorCommand::Redo);
    tester.expect_content(b"|");

    for byte in b"abcd" {
        tester.execute(EditorCommand::InsertText(std::slice::from_ref(byte)));
    }
    tester.expect_content(b"abcd|");
    tester.execute(EditorCommand::Undo);
    tester.execute(EditorCommand::Redo);
    for byte in b"efgh" {
        tester.execute(EditorCommand::InsertText(std::slice::from_ref(byte)));
    }
    tester.execute(EditorCommand::Undo);
    tester.expect_content(b"abcd|");
    tester.execute(EditorCommand::Undo);
    tester.expect_content(b"|");

    tester.execute(EditorCommand::ReplaceWholeFile(
        b"pub fn main() !void { return 5; }",
    ));
    tester.expect_content(b"pub fn main() !void { return 5; }|");
    tester.execute(EditorCommand::Click {
        position: tester.pos(29),
        mode: DragSelectionMode::default(),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"pub fn main() !void { return |5; }");
    tester.execute(EditorCommand::ReplaceWholeFile(
        b"pub fn main() !void {\n    return 5;\n}",
    ));
    tester.expect_content(b"pub fn main() !void {\n    return |5;\n}");
    tester.execute(EditorCommand::Undo);
    tester.expect_content(b"pub fn main() !void { return |5; }");

    while tester.editor.document().can_undo() {
        tester.execute(EditorCommand::Undo);
    }
    tester.expect_content(b"|hello!");
}
