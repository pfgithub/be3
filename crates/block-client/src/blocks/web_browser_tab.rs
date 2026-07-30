use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HistoryItem {
    pub url: String,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WebBrowserTab {
    history: Vec<HistoryItem>,
    index: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum WebBrowserTabOperation {
    Push(HistoryItem),
    Replace(HistoryItem),
    History(usize),
}

impl WebBrowserTab {
    pub fn new() -> Self {
        Self {
            history: vec![HistoryItem {
                url: "about:blank".into(),
                title: String::new(),
            }],
            index: 0,
        }
    }

    pub fn history(&self) -> &[HistoryItem] {
        &self.history
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn current(&self) -> &HistoryItem {
        &self.history[self.index]
    }

    pub fn can_go_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.index + 1 < self.history.len()
    }
}

impl Default for WebBrowserTab {
    fn default() -> Self {
        Self::new()
    }
}

impl Block for WebBrowserTab {
    type Operation = WebBrowserTabOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7765_622d_6272_6f77_7365_722d_7461_6201);

    fn apply_operation(tab: &mut Self, operation: &Self::Operation) {
        match operation {
            WebBrowserTabOperation::Push(item) => {
                tab.history.truncate(tab.index.saturating_add(1));
                tab.history.push(item.clone());
                tab.index = tab.history.len() - 1;
            }
            WebBrowserTabOperation::Replace(item) => {
                if let Some(current) = tab.history.get_mut(tab.index) {
                    current.clone_from(item);
                }
            }
            WebBrowserTabOperation::History(index) => {
                if *index < tab.history.len() {
                    tab.index = *index;
                }
            }
        }
    }

    fn implicit_name(&self) -> String {
        let title = self.current().title.trim();
        if title.is_empty() {
            "Web Browser Tab".into()
        } else {
            title.into()
        }
    }
}

#[cfg(test)]
#[path = "web_browser_tab/tests/web_browser_tab_history_changes_current_index.rs"]
mod web_browser_tab_history_changes_current_index;
#[cfg(test)]
#[path = "web_browser_tab/tests/web_browser_tab_new_starts_at_about_blank.rs"]
mod web_browser_tab_new_starts_at_about_blank;
#[cfg(test)]
#[path = "web_browser_tab/tests/web_browser_tab_push_appends_and_selects_url.rs"]
mod web_browser_tab_push_appends_and_selects_url;
#[cfg(test)]
#[path = "web_browser_tab/tests/web_browser_tab_push_discards_forward_history.rs"]
mod web_browser_tab_push_discards_forward_history;
#[cfg(test)]
#[path = "web_browser_tab/tests/web_browser_tab_replace_changes_current_url.rs"]
mod web_browser_tab_replace_changes_current_url;
#[cfg(test)]
#[path = "web_browser_tab/tests/web_browser_tab_title_determines_implicit_name.rs"]
mod web_browser_tab_title_determines_implicit_name;
