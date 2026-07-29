use block::Block;

use super::{HistoryItem, WebBrowserTab, WebBrowserTabOperation};

#[test]
fn web_browser_tab_replace_changes_current_url() {
    let mut tab = WebBrowserTab::new();
    WebBrowserTab::apply_operation(
        &mut tab,
        &WebBrowserTabOperation::Push(HistoryItem {
            url: "https://before.example".into(),
            title: String::new(),
        }),
    );

    WebBrowserTab::apply_operation(
        &mut tab,
        &WebBrowserTabOperation::Replace(HistoryItem {
            url: "https://after.example".into(),
            title: "After".into(),
        }),
    );

    assert_eq!(tab.history().len(), 2);
    assert_eq!(tab.current().url, "https://after.example");
    assert_eq!(tab.current().title, "After");
    assert_eq!(tab.index(), 1);
}
