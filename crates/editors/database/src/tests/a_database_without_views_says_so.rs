use super::*;

#[test]
fn a_database_without_views_says_so() {
    let (mut editor, _client, _block) = editor();

    editor.snapshot("a_database_without_views_says_so");
}
