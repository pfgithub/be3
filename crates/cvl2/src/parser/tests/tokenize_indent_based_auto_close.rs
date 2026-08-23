use super::*;

#[test]
fn tokenize_indent_based_auto_close() {
    let (result, _source) = tokenize_str("(\n    [a\n)");

    assert_eq!(result.errors.len(), 1);
    let entries = &result.errors[0].entries;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].message, "open bracket missing close bracket");
    assert!(entries[1].message.contains("expected \"]\""));
    assert!(entries[1].message.contains("indent '4'"));
    assert!(entries[1].message.contains("got \")\""));
    assert!(entries[1].message.contains("indent '0'"));

    assert_eq!(result.result.len(), 1);
    let SyntaxNode::Block(outer) = &result.result[0] else {
        panic!("expected a block, got {:?}", result.result[0]);
    };
    assert_eq!(outer.start, "(");
    assert_eq!(outer.end, ")");

    let is_map_block =
        |node: &SyntaxNode| matches!(node, SyntaxNode::Block(b) if b.tag == BracketTag::Map);
    assert_eq!(count_all(&result.result, &is_map_block), 1);
    assert!(find_first(
        &result.result,
        &|n| matches!(n, SyntaxNode::Identifier(t) if t.str == "a")
    )
    .is_some());
}
