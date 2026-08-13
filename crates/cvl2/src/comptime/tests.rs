use super::*;
use crate::compiler::{BlockIdx, ComptimeTypeVoid, DestructureExtract};
use crate::parser::IdentifierToken;

fn new_env() -> Env {
    Env {
        trace: Vec::new(),
        errors: Vec::new(),
        comptime: HashMap::new(),
    }
}

fn pos_at(idx: usize) -> TokenPosition {
    TokenPosition {
        fyl: "test".to_string(),
        idx,
        lyn: 1,
        col: idx + 1,
    }
}

fn ident_node(name: &str, idx: usize) -> SyntaxNode {
    SyntaxNode::Identifier(IdentifierToken {
        pos: pos_at(idx),
        str: name.to_string(),
        ident_tag: IdentifierTag::Normal,
        ident_tag_raw: String::new(),
    })
}

fn comptime_ast(idx: usize) -> ComptimeValueAst {
    ComptimeValueAst {
        ast: vec![ident_node("a", idx)],
        pos: pos_at(idx),
    }
}

fn string_key(key: &str) -> ComptimeNarrowKey {
    ComptimeNarrowKey::String {
        key: key.to_string(),
    }
}

fn void_type() -> ComptimeType {
    ComptimeType::Void(ComptimeTypeVoid { pos: pos_at(0) })
}

mod comptime_eval_evaluates_comptime_ast_and_key_lines;
mod comptime_eval_evaluates_void_and_comptime_only_lines;
mod comptime_eval_ns_list_append_duplicate_reports_error;
mod comptime_eval_ns_list_init_and_append_registers_field;
mod comptime_eval_unhandled_line_panics;
mod dump_ast_node_list_renders_identifier;
mod dump_block_renders_line_index_and_tag;
mod dump_destructure_renders_extract_and_type;
mod dump_type_renders_void_type;
