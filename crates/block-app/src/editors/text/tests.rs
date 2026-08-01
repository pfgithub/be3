use super::{format_embed, parse_embeds};
use uuid::Uuid;

const BLOCK_ID: Uuid = Uuid::from_u128(0x1234_5678_1234_5678_1234_5678_1234_5678);

fn block_url() -> String {
    format!("https://blocks.pfg.pw/0/{BLOCK_ID}")
}

mod classifies_inline_and_multiple_directives;
mod formats_block_url;
mod parses_full_line_directive_with_whitespace;
mod rejects_malformed_directives;
