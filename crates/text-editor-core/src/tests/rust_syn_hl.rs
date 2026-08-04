use super::*;

#[test]
fn rust_syn_hl() {
    let mut tester = EditorTester::with_language(b"const MAX: usize = 10;", Language::Rust);
    assert_eq!(
        rendered_highlight(&mut tester, 0),
        "<keyword_storage>const <variable_constant>MAX<punctuation_important>: <keyword_primitive_type>usize <keyword>= <literal>10<punctuation>;"
    );

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(
        b"let s = \"x: \\x5A, n: \\n, bs: \\\\, q: \\\", u: \\u{1F600}\";",
    ));
    assert_eq!(
        rendered_highlight(&mut tester, 0),
        "<keyword_storage>let <variable>s <keyword>= <punctuation>\"<literal_string>x: <punctuation>\\<keyword_storage>x<literal>5A<literal_string>, n: <punctuation>\\<literal>n<literal_string>, bs: <punctuation>\\<literal_string>\\, q: <punctuation>\\<literal_string>\", u: <punctuation>\\<keyword_storage>u<punctuation>{<literal>1F600<punctuation>}\";"
    );

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(
        b"//! a\n// b\n/// c\n/* d */\n/** e */",
    ));
    assert_eq!(
        rendered_highlight(&mut tester, 0),
        "<keyword>//! <markdown_plain_text>a\n<punctuation>// <comment>b\n<keyword>/// <markdown_plain_text>c\n<punctuation>/* <comment>d <punctuation>*/\n<keyword>/** <markdown_plain_text>e <keyword>*/"
    );

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(b"let raw = r#\"a\\n{}\"#;"));
    assert_eq!(
        rendered_highlight(&mut tester, 0),
        "<keyword_storage>let <variable>raw <keyword>= <keyword_storage>r<punctuation>#\"<literal_string>a\\n{}<punctuation>\"#;"
    );

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(b"fn go() { println!(\"hi\"); }"));
    assert_eq!(
        rendered_highlight(&mut tester, 0),
        "<keyword_storage>fn <variable_function>go<punctuation>() { <variable_function>println!<punctuation>(\"<literal_string>hi<punctuation>\"); }"
    );

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(
        b"let c = 'a'; let e = '\\n'; fn f<'x>() {}",
    ));
    assert_eq!(
        rendered_highlight(&mut tester, 0),
        "<keyword_storage>let <variable>c <keyword>= <punctuation>'<literal_string>a<punctuation>'; <keyword_storage>let <variable>e <keyword>= <punctuation>'\\<literal>n<punctuation>'; <keyword_storage>fn <variable_function>f<keyword><'x><punctuation>() {}"
    );

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(
        b"#[derive(Debug)] struct S { a: u8 }",
    ));
    assert_eq!(
        rendered_highlight(&mut tester, 0),
        "<keyword>#[<variable_function>derive<punctuation>(<keyword_primitive_type>Debug<punctuation>)] <keyword>struct <keyword_primitive_type>S <punctuation>{ <variable>a<punctuation_important>: <keyword_primitive_type>u8 <punctuation>}"
    );

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(b"let mut n = 0x1F_u8;"));
    assert_eq!(
        rendered_highlight(&mut tester, 0),
        "<keyword_storage>let mut <variable_mutable>n <keyword>= <keyword_storage>0x<literal>1F_<keyword_primitive_type>u8<punctuation>;"
    );
}
