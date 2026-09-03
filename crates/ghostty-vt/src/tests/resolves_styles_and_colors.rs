use super::*;

#[test]
fn resolves_styles_and_colors() {
    let screen = render(20, 2, "\u{1b}[1;31mR\u{1b}[0m\u{1b}[7mI");

    let bold = &screen.rows[0].cells[0];
    assert!(bold.bold);
    assert_ne!(bold.foreground, screen.foreground);
    assert_eq!(bold.background, None);

    let inverse = &screen.rows[0].cells[1];
    assert_eq!(inverse.text, "I");
    assert_eq!(inverse.foreground, screen.background);
    assert_eq!(inverse.background, Some(screen.foreground));
}
