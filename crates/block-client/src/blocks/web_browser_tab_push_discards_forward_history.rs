use block::Block;

use super::web_browser_tab::{HistoryItem, WebBrowserTab, WebBrowserTabOperation};

#[test]
fn web_browser_tab_push_discards_forward_history() {
    let mut tab = WebBrowserTab::new();
    for operation in [
        WebBrowserTabOperation::Push(HistoryItem {
            url: "https://one.example".into(),
            title: String::new(),
        }),
        WebBrowserTabOperation::Push(HistoryItem {
            url: "https://two.example".into(),
            title: String::new(),
        }),
        WebBrowserTabOperation::History(1),
        WebBrowserTabOperation::Push(HistoryItem {
            url: "https://three.example".into(),
            title: String::new(),
        }),
    ] {
        WebBrowserTab::apply_operation(&mut tab, &operation);
    }

    let urls: Vec<_> = tab.history().iter().map(|item| item.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "about:blank",
            "https://one.example",
            "https://three.example"
        ]
    );
    assert_eq!(tab.index(), 2);
}
