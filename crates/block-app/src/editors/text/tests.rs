use super::{format_embed, parse_embeds};
use uuid::Uuid;

const BLOCK_ID: Uuid = Uuid::from_u128(0x1234_5678_1234_5678_1234_5678_1234_5678);

fn directive(settings: &str) -> String {
    format!("{{{{_BLOCKEDITOR:{BLOCK_ID}:{settings}}}}}")
}

mod classifies_inline_and_multiple_directives;
mod formats_empty_settings_directive;
mod parses_full_line_directive_with_whitespace;
mod preserves_opaque_settings;
mod rejects_malformed_directives;
