use block::Block;

use super::web_browser_tab::{HistoryItem, WebBrowserTab, WebBrowserTabOperation};

#[test]
fn web_browser_tab_history_changes_current_index() {
    let mut tab = WebBrowserTab::new();
    WebBrowserTab::apply_operation(
        &mut tab,
        &WebBrowserTabOperation::Push(HistoryItem {
            url: "https://example.com".into(),
            title: String::new(),
        }),
    );

    WebBrowserTab::apply_operation(&mut tab, &WebBrowserTabOperation::History(0));
    assert_eq!(tab.index(), 0);
    assert!(tab.can_go_forward());

    WebBrowserTab::apply_operation(&mut tab, &WebBrowserTabOperation::History(99));
    assert_eq!(tab.index(), 0);
}
