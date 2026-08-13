use super::*;

#[test]
fn pretty_print_errors_renders_new_error_styles() {
    let source = Source::new("f.cvl2", "abc");
    let error = TokenizationError {
        entries: vec![
            TokenizationErrorEntry {
                pos: None,
                style: ErrorStyle::Todo,
                message: "todo msg".to_string(),
            },
            TokenizationErrorEntry {
                pos: None,
                style: ErrorStyle::Warning,
                message: "warning msg".to_string(),
            },
            TokenizationErrorEntry {
                pos: None,
                style: ErrorStyle::Unreachable,
                message: "unreachable msg".to_string(),
            },
        ],
        trace: Vec::new(),
    };

    let output = pretty_print_errors(&[&source], std::slice::from_ref(&error));
    assert!(output.contains("todo"));
    assert!(output.contains("warning"));
    assert!(output.contains("unreachable"));
    assert!(output.contains(colors::BLUE));
    assert!(output.contains(colors::YELLOW));
    assert!(output.contains(colors::BRBLACK));
}
