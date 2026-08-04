use super::*;

#[test]
fn zig_syn_hl() {
    let mut tester = EditorTester::new(b"const std = @import(\"std\");");
    assert_eq!(
        rendered_highlight(&mut tester, 0),
        "<keyword_storage>const <variable_constant>std <keyword>= @import<punctuation>(\"<literal_string>std<punctuation>\");"
    );
    assert_eq!(
        rendered_highlight(&mut tester, 19),
        "<punctuation>(\"<literal_string>std<punctuation>\");"
    );

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(
        b"const mystr = \"x_esc: \\x5A, n_esc: \\n, bks_esc: \\\\, str_esc: \\\", u_esc: \\u{ABC123}\";",
    ));
    assert_eq!(
        rendered_highlight(&mut tester, 0),
        "<keyword_storage>const <variable_constant>mystr <keyword>= <punctuation>\"<literal_string>x_esc: <punctuation>\\<keyword_storage>x<literal>5A<literal_string>, n_esc: <punctuation>\\<literal>n<literal_string>, bks_esc: <punctuation>\\<literal_string>\\, str_esc: <punctuation>\\<literal_string>\", u_esc: <punctuation>\\<keyword_storage>u<punctuation>{<literal>ABC123<punctuation>}\";"
    );

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(
        b"//!c1\n//c2\n///c3\nconst a = 0;",
    ));
    assert_eq!(
        rendered_highlight(&mut tester, 0),
        "<keyword>//!<markdown_plain_text>c1\n<punctuation>//<comment>c2\n<keyword>///<markdown_plain_text>c3\n<keyword_storage>const <variable_constant>a <keyword>= <literal>0<punctuation>;"
    );

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(
        b"const a = \"Hello {s}: {{.a = b}}\";",
    ));
    assert_eq!(
        rendered_highlight(&mut tester, 10),
        "<punctuation>\"<literal_string>Hello <punctuation>{<literal_string>s<punctuation>}<literal_string>: <punctuation>{<literal_string>{.a = b}<punctuation>}\";"
    );
}
