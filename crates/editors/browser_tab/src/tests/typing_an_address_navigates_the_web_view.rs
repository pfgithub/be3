use super::*;

#[test]
fn typing_an_address_navigates_the_web_view() {
    let Fixture {
        mut editor,
        host,
        block,
    } = editor();
    let _ = host.take_web_view_commands();

    editor.find("browser.address").click();
    editor.run();
    editor.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    editor.run();
    editor.find("browser.address").type_text("example.org");
    editor.run();
    editor.find("browser.go").click();
    editor.run();

    assert_eq!(
        urls(&block).last().map(String::as_str),
        Some("https://example.org")
    );
    assert!(host
        .take_web_view_commands()
        .contains(&WebViewCommand::Load("https://example.org".into())));
    editor.snapshot("typing_an_address_navigates_the_web_view");
}
