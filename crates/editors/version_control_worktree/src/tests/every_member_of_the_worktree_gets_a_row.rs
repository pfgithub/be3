use super::*;

#[test]
fn every_member_of_the_worktree_gets_a_row() {
    let Fixture {
        mut editor,
        members,
    } = editor(2);

    for member in &members {
        editor.find(&format!("worktree.member.{member}"));
    }
    editor.snapshot("every_member_of_the_worktree_gets_a_row");
}
