use block_client::block_url;
use uuid::Uuid;

use super::{find_matches, image_embed_directive, parse_embeds};

mod classifies_markdown_image;
mod find_matches_is_case_insensitive_by_default;
mod find_matches_matches_are_non_overlapping;
mod find_matches_respects_case_sensitive_flag;
mod find_matches_returns_empty_for_empty_query;
mod foreign_workspace_url_is_not_an_embed;
mod image_embed_directive_uses_markdown_image;
mod image_embed_directive_uses_plain_url;
mod parses_markdown_checkboxes;

const BLOCK_ID: Uuid = Uuid::from_u128(0xe2b8_7b59_9c69_4d75_83fd_801b_2727_1388);
const WORKSPACE_ID: Uuid = Uuid::from_u128(0x7a20_a314_e4aa_4ca7_b7ae_d68c_3249_0d9d);
