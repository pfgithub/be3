use block_client::{blocks::text::TextDocument, BlockClient};
use uuid::Uuid;

use super::*;

mod core;
mod ctrl_d;
mod has_stop;
mod markdown_block_styles;
mod markdown_incremental_edit;
mod markdown_inline_styles;
mod markdown_invalid_utf8;
mod raw_bytes;
mod zig_syn_hl;

struct EditorTester {
    _client: BlockClient,
    editor: Core,
}

impl EditorTester {
    fn new(initial: impl AsRef<[u8]>) -> Self {
        Self::with_language(initial, Language::Zig)
    }

    fn with_language(initial: impl AsRef<[u8]>, language: Language) -> Self {
        let client = BlockClient::new(Uuid::new_v4());
        let block = client.create_block(TextDocument::from_bytes(initial));
        let mut editor = Core::new(block);
        editor.set_syntax_highlighter(Some(language));
        Self {
            _client: client,
            editor,
        }
    }

    fn pos(&self, byte: usize) -> Position {
        self.editor.position(byte)
    }

    fn execute(&mut self, command: EditorCommand<'_>) {
        self.editor.execute_command(command);
    }

    fn expect_content(&self, expected: impl AsRef<[u8]>) {
        let document = self.editor.document().read().unwrap();
        let mut markers = Vec::new();
        for cursor in self.editor.cursor_positions() {
            let anchor = cursor.pos.anchor.resolve(&document);
            let focus = cursor.pos.focus.resolve(&document);
            if anchor == focus {
                markers.push((focus, b'|'));
            } else if anchor < focus {
                markers.push((anchor, b'['));
                markers.push((focus, b'|'));
            } else {
                markers.push((focus, b'|'));
                markers.push((anchor, b']'));
            }
        }
        markers.sort_by_key(|(index, _)| *index);
        let mut actual = Vec::new();
        for index in 0..=document.len() {
            actual.extend(
                markers
                    .iter()
                    .filter(|(marker_index, _)| *marker_index == index)
                    .map(|(_, marker)| *marker),
            );
            if let Some(byte) = document.bytes().get(index) {
                actual.push(*byte);
            }
        }
        assert_eq!(
            actual,
            expected.as_ref(),
            "cursors: {:?}",
            self.editor.cursor_positions()
        );
    }
}
