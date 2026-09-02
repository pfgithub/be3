use std::sync::Arc;

use block_client::blocks::web_browser_tab::{HistoryItem, WebBrowserTab, WebBrowserTabOperation};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{egui, App as _, EditorHost, WebViewCommand, WebViewEvent};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::BrowserTabApp;

mod a_pushed_url_becomes_the_tab_s_history;
mod typing_an_address_navigates_the_web_view;

const ACCOUNT: Uuid = Uuid::from_u128(0x7765_622d_7465_7374_2d61_6363_6f75_6e74);
const WORKSPACE: Uuid = Uuid::from_u128(0x7765_622d_7465_7374_2d77_6f72_6b73_7061);

struct Fixture {
    editor: EditorTest<'static, BrowserTabApp>,
    host: EditorHost,
    block: BlockHandle<WebBrowserTab>,
}

fn editor() -> Fixture {
    let client = Arc::new(BlockClient::new(ACCOUNT, WORKSPACE));
    let block = client.create_block(WebBrowserTab::new());
    block.operate(WebBrowserTabOperation::Replace(HistoryItem {
        url: "https://example.com/".into(),
        title: String::new(),
    }));
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(ACCOUNT);
    let mut app = BrowserTabApp::default();
    app.connect(host.clone(), client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    Fixture {
        editor,
        host,
        block,
    }
}

fn urls(block: &BlockHandle<WebBrowserTab>) -> Vec<String> {
    block
        .read()
        .unwrap()
        .history()
        .iter()
        .map(|item| item.url.clone())
        .collect()
}
