use super::*;

#[test]
fn a_painting_that_was_never_approved_is_new() {
    let (review, mut editor) = Review::open();
    assert_eq!(status(&mut editor, PATH), Some(Status::New));

    editor.find(&entry_id(PATH)).click();
    editor.run();
    editor.snapshot("a_new_painting_is_shown_for_review");

    editor.find("paint_review.approve").click();
    editor.run();

    assert_eq!(status(&mut editor, PATH), Some(Status::Unchanged));
    assert_eq!(review.approvals(), 1);
    assert_eq!(
        review.approved(PATH).unwrap().data(),
        painting(30).encode().unwrap()
    );
}
