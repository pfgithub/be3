use super::*;

#[test]
fn pretty_print_errors_skips_line_for_ambiguous_duplicate_filename() {
    let source_a = Source::new("dup.cvl2", "line one");
    let source_b = Source::new("dup.cvl2", "line one");
    let error = TokenizationError {
        entries: vec![TokenizationErrorEntry {
            pos: Some(TokenPosition {
                fyl: "dup.cvl2".to_string(),
                idx: 0,
                lyn: 1,
                col: 1,
            }),
            style: ErrorStyle::Error,
            message: "boom".to_string(),
        }],
        trace: Vec::new(),
    };

    let output = pretty_print_errors(&[&source_a, &source_b], std::slice::from_ref(&error));
    assert!(output.contains("boom"));
    assert!(!output.contains("line one"));

    let output_single = pretty_print_errors(&[&source_a], std::slice::from_ref(&error));
    assert!(output_single.contains("line one"));
}
