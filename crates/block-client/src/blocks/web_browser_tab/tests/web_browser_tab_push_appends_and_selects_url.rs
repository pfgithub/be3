use block::Block;

use super::{HistoryItem, WebBrowserTab, WebBrowserTabOperation};

#[test]
fn web_browser_tab_push_appends_and_selects_url() {
    let mut tab = WebBrowserTab::new();

    WebBrowserTab::apply_operation(
        &mut tab,
        &WebBrowserTabOperation::Push(HistoryItem {
            url: "https://example.com".into(),
            title: String::new(),
        }),
    );

    assert_eq!(tab.history().len(), 2);
    assert_eq!(tab.current().url, "https://example.com");
    assert_eq!(tab.index(), 1);
    assert!(tab.can_go_back());
    assert!(!tab.can_go_forward());
}
