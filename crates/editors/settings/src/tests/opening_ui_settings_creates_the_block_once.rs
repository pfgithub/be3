use super::*;

#[test]
fn opening_ui_settings_creates_the_block_once() {
    let (mut editor, block) = editor();

    editor.find("settings.ui-settings").click();
    editor.run();
    let created = block
        .read()
        .unwrap()
        .entries(UiSettings::TYPE_ID)
        .first()
        .map(|entry| entry.block);
    assert!(created.is_some());

    editor.find("settings.ui-settings").click();
    editor.run();
    assert_eq!(block.read().unwrap().entries(UiSettings::TYPE_ID).len(), 1);
    editor.snapshot("opening_ui_settings_creates_the_block_once");
}
