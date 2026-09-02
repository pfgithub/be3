use std::sync::Arc;

use block_client::blocks::text::TextDocument;
use block_client::{block_ref::BlockRef, block_url, BlockClient, BlockHandle};
use block_editor_plugin::App as _;
use block_ui_test::EditorTest;
use uuid::Uuid;

use super::{image_embed_directive, parse_embeds, TextApp};

mod classifies_markdown_image;
mod foreign_workspace_url_is_not_an_embed;
mod image_embed_directive_uses_markdown_image;
mod image_embed_directive_uses_plain_url;
mod parses_markdown_checkboxes;
mod replacing_a_referenced_block_rewrites_its_url;
mod switching_to_hex_view_shows_the_bytes;
mod the_intrinsic_size_follows_the_width_it_was_given;

fn editor(text: &str) -> (EditorTest<'static, TextApp>, BlockHandle<TextDocument>) {
    let client = Arc::new(BlockClient::new(ACCOUNT_ID, WORKSPACE_ID));
    let block = client.create_block(TextDocument::new());
    let mut core = text_editor_core::Core::new(block.clone());
    let start = core.position(0);
    core.execute_command(text_editor_core::EditorCommand::SetSelection {
        anchor: start,
        focus: start,
    });
    core.execute_command(text_editor_core::EditorCommand::InsertText(text.as_bytes()));
    let mut app = TextApp::default();
    app.connect(Default::default(), client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}

fn text(block: &BlockHandle<TextDocument>) -> String {
    block
        .read()
        .expect("the document is loaded")
        .text_lossy()
        .into_owned()
}

const ACCOUNT_ID: Uuid = Uuid::from_u128(0x11ac_c001_0000_4000_8000_0000_0000_0001);
const BLOCK_ID: Uuid = Uuid::from_u128(0xe2b8_7b59_9c69_4d75_83fd_801b_2727_1388);
const WORKSPACE_ID: Uuid = Uuid::from_u128(0x7a20_a314_e4aa_4ca7_b7ae_d68c_3249_0d9d);
