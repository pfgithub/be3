use super::*;

pub(super) const DOCUMENT: &str = r#"{
    "id": "be3.counter",
    "name": "Counter",
    "version": "0.1.0",
    "block_type": "636f756e-7465-722d-626c-6f636b2d0001",
    "display_name": "Counter",
    "icon": "\ueb8d",
    "creation": "Immediate",
    "regions": ["Main", "Toolbar"],
    "entry_points": { "web": "counter.js" },
    "surfaces": ["WebExternalImage"]
}"#;

mod a_wasm_entry_point_needs_a_host_texture_surface;
mod manifest_document_defaults_what_the_host_can;
mod manifest_document_rejects_a_bad_block_type;
mod manifest_document_rejects_unknown_fields;
mod manifest_from_json_reads_a_document;
