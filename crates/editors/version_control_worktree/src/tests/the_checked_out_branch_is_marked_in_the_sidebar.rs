use super::*;

#[test]
fn the_checked_out_branch_is_marked_in_the_sidebar() {
    let Fixture { mut editor, .. } = editor(1);

    assert!(editor
        .try_find(&format!("worktree.switch.{MAIN_BRANCH}"))
        .is_none());
    editor.snapshot("the_checked_out_branch_is_marked_in_the_sidebar");
}
