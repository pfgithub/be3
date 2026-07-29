use block::Block;

use super::{HistoryItem, WebBrowserTab, WebBrowserTabOperation};

#[test]
fn web_browser_tab_title_determines_implicit_name() {
    let mut tab = WebBrowserTab::new();
    assert_eq!(tab.implicit_name(), "Web Browser Tab");

    WebBrowserTab::apply_operation(
        &mut tab,
        &WebBrowserTabOperation::Replace(HistoryItem {
            url: "https://example.com".into(),
            title: "Example Domain".into(),
        }),
    );

    assert_eq!(tab.implicit_name(), "Example Domain");
}
