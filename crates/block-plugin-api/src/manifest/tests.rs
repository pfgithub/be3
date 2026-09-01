use super::*;

pub(super) const DOCUMENT: &str = r#"{
    "id": "be3.counter",
    "name": "Counter",
    "version": "0.1.0",
    "block_type": "636f756e-7465-722d-626c-6f636b2d0001",
    "display_name": "Counter",
    "icon": "\ueb8d",
    "creation": "Immediate",
    "regions": ["Frame"],
    "chrome": ["Toolbar"],
    "entry_point": "counter.wasm"
}"#;

mod an_empty_entry_point_is_rejected;
mod manifest_document_defaults_what_the_host_can;
mod manifest_document_rejects_a_bad_block_type;
mod manifest_document_rejects_unknown_fields;
mod manifest_from_json_reads_a_document;
