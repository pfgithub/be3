use super::*;

#[test]
fn a_pushed_url_becomes_the_tab_s_history() {
    let Fixture {
        mut editor,
        host,
        block,
        ..
    } = editor();

    host.push_web_view_event(WebViewEvent::Push("https://example.com/next".into()));
    editor.run();

    assert_eq!(
        urls(&block),
        ["https://example.com/", "https://example.com/next"]
    );
}
