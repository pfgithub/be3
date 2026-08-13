use super::*;

#[test]
fn tokenize_inline_comment_captures_until_newline() {
    let (result, _source) = tokenize_str("// hello\nabc");
    assert!(result.errors.is_empty());

    let is_comment_block =
        |n: &SyntaxNode| matches!(n, SyntaxNode::Block(b) if b.tag == BracketTag::InlineComment);
    let comment_block =
        find_first(&result.result, &is_comment_block).expect("expected a comment block");
    let SyntaxNode::Block(block) = comment_block else {
        unreachable!("find_first only matched blocks")
    };
    assert_eq!(block.start, "//");
    assert_eq!(block.items.len(), 1);
    match &block.items[0] {
        SyntaxNode::Raw(r) => {
            assert_eq!(r.raw, " hello");
            assert_eq!(r.tag, RawTag::Comment);
        }
        other => panic!("expected a raw comment token, got {other:?}"),
    }

    assert!(find_first(
        &result.result,
        &|n| matches!(n, SyntaxNode::Identifier(t) if t.str == "abc")
    )
    .is_some());
}
