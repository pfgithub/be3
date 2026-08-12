use super::*;

fn tokenize_str(text: &str) -> (TokenizationResult, Source) {
    let mut source = Source::new("test.cvl2", text);
    let result = tokenize(&mut source);
    (result, source)
}

fn find_first<'a>(
    nodes: &'a [SyntaxNode],
    predicate: &dyn Fn(&SyntaxNode) -> bool,
) -> Option<&'a SyntaxNode> {
    for node in nodes {
        if predicate(node) {
            return Some(node);
        }
        let children: Option<&[SyntaxNode]> = match node {
            SyntaxNode::Block(b) => Some(&b.items),
            SyntaxNode::BinaryExpression(b) => Some(&b.items),
            SyntaxNode::OperatorSegment(s) => Some(&s.items),
            _ => None,
        };
        if let Some(children) = children {
            if let Some(found) = find_first(children, predicate) {
                return Some(found);
            }
        }
    }
    None
}

fn count_all(nodes: &[SyntaxNode], predicate: &dyn Fn(&SyntaxNode) -> bool) -> usize {
    let mut count = 0;
    for node in nodes {
        if predicate(node) {
            count += 1;
        }
        let children: Option<&[SyntaxNode]> = match node {
            SyntaxNode::Block(b) => Some(&b.items),
            SyntaxNode::BinaryExpression(b) => Some(&b.items),
            SyntaxNode::OperatorSegment(s) => Some(&s.items),
            _ => None,
        };
        if let Some(children) = children {
            count += count_all(children, predicate);
        }
    }
    count
}

mod pretty_print_errors_renders_message_and_pointer;
mod source_cursor_tracks_position;
mod source_indent_level_tracks_leading_spaces;
mod source_revert_restores_position;
mod tokenize_access_and_builtin_identifiers;
mod tokenize_bad_token_reports_error;
mod tokenize_bind_operators;
mod tokenize_bracket_blocks;
mod tokenize_equals_assignment;
mod tokenize_extra_close_bracket_reports_error;
mod tokenize_identifier;
mod tokenize_in_string_backslash_is_unimplemented;
mod tokenize_indent_based_auto_close;
mod tokenize_mismatched_bracket_reports_error;
mod tokenize_mixing_bind_operators_reports_error;
mod tokenize_raw_tokens;
mod tokenize_sample_source_succeeds;
mod tokenize_separators_build_binary_expressions;
mod tokenize_string_produces_str_seg;
mod tokenize_whitespace_and_newline;
