use super::*;

#[test]
fn dump_ast_node_list_renders_identifier() {
    let nodes = vec![ident_node("hello", 0)];

    let dump = dump_ast_node_list(&nodes, usize::MAX);

    assert!(dump.contains("ident"));
    assert!(dump.contains("hello"));
    assert!(dump.contains("test:1:1"));
}
