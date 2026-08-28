use super::*;

#[test]
fn unapproving_a_painting_makes_it_new_again() {
    let (review, mut editor) = Review::open();
    editor.find(&entry_id(PATH)).click();
    editor.run();
    editor.find("paint_review.approve").click();
    editor.run();
    assert_eq!(status(&mut editor, PATH), Some(Status::Unchanged));
    let snapshot = review.reference(PATH).unwrap();

    editor.find("paint_review.unapprove").click();
    editor.run();

    assert_eq!(status(&mut editor, PATH), Some(Status::New));
    assert_eq!(review.approvals(), 0);
    assert!(review.orphaned(snapshot));

    editor.find("paint_review.approve").click();
    editor.run();
    assert_eq!(status(&mut editor, PATH), Some(Status::Unchanged));
    assert_eq!(
        review.approved(PATH).unwrap().data(),
        painting(30).encode().unwrap()
    );
}
