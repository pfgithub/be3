use super::web_browser_tab::WebBrowserTab;

#[test]
fn web_browser_tab_new_starts_at_about_blank() {
    let tab = WebBrowserTab::new();

    assert_eq!(tab.history().len(), 1);
    assert_eq!(tab.current().url, "about:blank");
    assert_eq!(tab.current().title, "");
    assert_eq!(tab.index(), 0);
    assert!(!tab.can_go_back());
    assert!(!tab.can_go_forward());
}
