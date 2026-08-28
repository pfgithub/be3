use super::*;

#[test]
fn deleting_a_snapshot_forgets_the_path_it_held() {
    let id = Uuid::new_v4();
    let mut review = PaintReview::new();
    PaintReview::apply_operation(
        &mut review,
        &PaintReviewOperation::Approve {
            painting: ApprovedPainting {
                path: "a.paint".to_owned(),
                hash: "hash".to_owned(),
                snapshot: BlockRef::Direct(id),
            },
        },
    );
    let operations = review.delete_child(id).unwrap();
    for operation in &operations {
        PaintReview::apply_operation(&mut review, operation);
    }
    assert!(review.approved().is_empty());
    assert!(review.delete_child(Uuid::new_v4()).unwrap().is_empty());
}
