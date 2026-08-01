use block_client::block_url;
use uuid::Uuid;

use super::parse_embeds;

mod classifies_markdown_image;
mod parses_markdown_checkboxes;

const BLOCK_ID: Uuid = Uuid::from_u128(0xe2b8_7b59_9c69_4d75_83fd_801b_2727_1388);
