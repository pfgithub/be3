use super::*;

#[test]
fn approving_a_path_again_replaces_what_was_approved() {
    let snapshot = BlockRef::Direct(Uuid::new_v4());
    let mut review = PaintReview::new();
    for hash in ["first", "second"] {
        PaintReview::apply_operation(
            &mut review,
            &PaintReviewOperation::Approve {
                painting: ApprovedPainting {
                    path: "a.paint".to_owned(),
                    hash: hash.to_owned(),
                    snapshot,
                },
            },
        );
    }
    assert_eq!(review.approved().len(), 1);
    assert_eq!(review.approval("a.paint").unwrap().hash, "second");
    assert_eq!(review.references(), [snapshot.as_direct().unwrap()]);
}
