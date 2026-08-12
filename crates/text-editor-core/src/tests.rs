use block_client::{
    blocks::text::{TextDocument, TextLanguage},
    BlockClient,
};
use uuid::Uuid;

use super::*;

mod block_urls_are_single_cursor_units;
mod collapse_and_uncollapse_affect_touched_lines;
mod collapsed_section_reveals_temporarily_around_the_cursor;
mod collapsible_sections_detects_indentation_blocks;
mod collapsible_sections_detects_markdown_headings;
mod core;
mod ctrl_d;
mod down_arrow_skips_past_a_collapsed_section;
mod find_matches_is_case_insensitive_by_default;
mod find_matches_matches_are_non_overlapping;
mod find_matches_respects_case_sensitive_flag;
mod find_matches_returns_empty_for_empty_query;
mod find_next_cycles_through_matches_and_wraps;
mod find_previous_cycles_through_matches_and_wraps;
mod find_status_reports_current_match_index;
mod has_stop;
mod markdown_block_styles;
mod markdown_checkbox_toggle;
mod markdown_formatting;
mod markdown_formatting_toggles_off;
mod markdown_incremental_edit;
mod markdown_inline_code_escapes_backtick_runs;
mod markdown_inline_code_pads_backtick_edges;
mod markdown_inline_code_toggle_off;
mod markdown_inline_styles;
mod markdown_invalid_utf8;
mod markdown_list_newline;
mod markdown_plain_punctuation;
mod markdown_tables;
mod raw_bytes;
mod replace_all_matches_does_nothing_when_there_are_no_matches;
mod replace_all_matches_replaces_every_match;
mod replace_match_does_nothing_when_selection_is_not_on_a_match;
mod replace_match_replaces_current_and_advances_to_next_match;
mod right_arrow_opens_a_collapsed_line;
mod rust_syn_hl;
mod toggle_collapse_at_is_independent_of_the_cursor;
mod toggle_collapse_at_leaves_a_cursor_outside_the_section_untouched;
mod toggle_collapse_at_moves_the_cursor_out_of_a_newly_hidden_section;
mod up_arrow_skips_past_a_collapsed_section;
mod zig_syn_hl;

/// Renders the document with `<scope>` markers wherever the colour scope
/// changes, skipping whitespace so that trailing-scope inheritance is ignored.
fn rendered_highlight(tester: &mut EditorTester, offset: usize) -> String {
    let highlight = tester.editor.highlight();
    let document = tester.editor.document().read().unwrap();
    let mut result = String::new();
    let mut previous = SynHlColorScope::Invalid;
    for index in offset..document.len() {
        let scope = highlight.advance_and_read(index);
        let byte = document.bytes()[index];
        if !byte.is_ascii_whitespace() && scope != previous {
            result.push('<');
            result.push_str(scope.as_str());
            result.push('>');
        }
        previous = scope;
        result.push(char::from(byte));
    }
    result
}

struct EditorTester {
    _client: BlockClient,
    editor: Core,
}

impl EditorTester {
    fn new(initial: impl AsRef<[u8]>) -> Self {
        Self::with_language(initial, TextLanguage::Zig)
    }

    fn with_language(initial: impl AsRef<[u8]>, language: TextLanguage) -> Self {
        let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
        let block = client.create_block(TextDocument::from_bytes(initial));
        let mut editor = Core::new(block);
        editor.set_language(language);
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

    fn set_cursor(&mut self, position: Position) {
        self.execute(EditorCommand::SetSelection {
            anchor: position,
            focus: position,
        });
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
