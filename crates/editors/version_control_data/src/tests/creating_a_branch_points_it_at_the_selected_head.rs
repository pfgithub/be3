use super::*;

#[test]
fn creating_a_branch_points_it_at_the_selected_head() {
    let (mut editor, block) = editor();

    editor.find("repository.new-branch-name").focus();
    editor.find("repository.new-branch-name").type_text("topic");
    editor.run();
    editor.find("repository.create-branch").click();
    editor.run();

    let data = block.read().unwrap();
    assert_eq!(data.branch_head("topic"), data.branch_head("main"));
    drop(data);
    editor.snapshot("creating_a_branch_points_it_at_the_selected_head");
}
