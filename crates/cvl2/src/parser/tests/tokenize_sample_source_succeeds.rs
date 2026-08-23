use super::*;

const SAMPLE_SOURCE: &str = "abc [
    def [jkl]
    if (
            amazing.one()
    ] else {
            wow!
    }
    demoFn(1, 2
        3
        4, 5, 6
    7, 8)
    commaExample(1, 2, 3, 4)
    colonExample(a: 1, b: c: 2, 3)
    newlineCommaExample(
        1, 2
        3
        4
    )
    (a, b => c, d = e, f => g, h)
] ghi";

#[test]
fn tokenize_sample_source_succeeds() {
    let (result, _source) = tokenize_str(SAMPLE_SOURCE);

    assert!(!result.result.is_empty());
    assert!(!result.errors.is_empty());
    assert!(find_first(
        &result.result,
        &|n| matches!(n, SyntaxNode::Identifier(t) if t.str == "abc")
    )
    .is_some());
    assert!(find_first(
        &result.result,
        &|n| matches!(n, SyntaxNode::Identifier(t) if t.str == "ghi")
    )
    .is_some());
}
