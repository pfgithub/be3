use super::*;

#[test]
fn a_painting_that_vanished_is_removed() {
    let (review, mut editor) = Review::open();
    editor.find(&entry_id(PATH)).click();
    editor.run();
    editor.find("paint_review.approve").click();
    editor.run();

    review.remove(PATH);
    editor.find("paint_review.refresh").click();
    editor.run();
    assert_eq!(status(&mut editor, PATH), Some(Status::Removed));
    assert!(review.approved(PATH).is_some());

    editor.find("paint_review.unapprove").click();
    editor.run();
    assert_eq!(status(&mut editor, PATH), None);
    assert_eq!(review.approvals(), 0);
}
