use super::*;

#[test]
fn pretty_print_errors_renders_message_and_pointer() {
    let (result, source) = tokenize_str("#");
    assert_eq!(result.errors.len(), 1);

    let output = pretty_print_errors(&source, &result.errors);
    assert!(output.contains("test.cvl2:1:1"));
    assert!(output.contains("error"));
    assert!(output.contains("bad token"));
    assert!(output.contains('^'));
    assert!(output.contains(colors::RED));
    assert!(output.contains(colors::RESET));

    assert_eq!(pretty_print_errors(&source, &[]), "");
}
