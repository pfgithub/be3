use super::*;

#[test]
fn a_painting_that_changed_on_the_branch_is_modified() {
    let (review, mut editor) = Review::open();
    editor.find(&entry_id(PATH)).click();
    editor.run();
    editor.find("paint_review.approve").click();
    editor.run();
    let snapshot = review.reference(PATH);

    review.write(PATH, &painting(200));
    editor.find("paint_review.refresh").click();
    editor.run();
    assert_eq!(status(&mut editor, PATH), Some(Status::Modified));

    editor.find("paint_review.view.approved").click();
    editor.run();
    editor.find("paint_review.view.current").click();
    editor.run();

    editor.find("paint_review.approve").click();
    editor.run();
    assert_eq!(status(&mut editor, PATH), Some(Status::Unchanged));
    assert_eq!(review.approvals(), 1);
    assert_eq!(review.reference(PATH), snapshot);
    assert_eq!(
        review.approved(PATH).unwrap().data(),
        painting(200).encode().unwrap()
    );
}
