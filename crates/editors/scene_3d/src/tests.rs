use std::sync::Arc;

use block_client::blocks::scene_3d::Scene3D;
use block_client::BlockClient;
use block_editor_plugin::{egui, App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::Scene3DApp;

mod clicking_the_scene_grabs_the_cursor_and_escape_releases_it;

const ACCOUNT: Uuid = Uuid::from_u128(0x3364_2d74_6573_742d_6163_636f_756e_7401);
const WORKSPACE: Uuid = Uuid::from_u128(0x3364_2d74_6573_742d_776f_726b_7370_6101);

fn editor() -> (EditorTest<'static, Scene3DApp>, EditorHost) {
    let client = Arc::new(BlockClient::new(ACCOUNT, WORKSPACE));
    let block = client.create_block(Scene3D::new());
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(ACCOUNT);
    let mut app = Scene3DApp::default();
    app.connect(host.clone(), client, block.id());
    let mut editor = EditorTest::new(app);
    editor.step();
    editor.step();
    (editor, host)
}
