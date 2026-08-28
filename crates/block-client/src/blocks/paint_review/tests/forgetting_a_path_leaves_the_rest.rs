use super::*;

#[test]
fn forgetting_a_path_leaves_the_rest() {
    let mut review = PaintReview::new();
    for path in ["a.paint", "b.paint"] {
        PaintReview::apply_operation(
            &mut review,
            &PaintReviewOperation::Approve {
                painting: ApprovedPainting {
                    path: path.to_owned(),
                    hash: "hash".to_owned(),
                    snapshot: BlockRef::Direct(Uuid::new_v4()),
                },
            },
        );
    }
    PaintReview::apply_operation(
        &mut review,
        &PaintReviewOperation::Forget {
            path: "a.paint".to_owned(),
        },
    );
    assert!(review.approval("a.paint").is_none());
    assert_eq!(review.approved().len(), 1);
    PaintReview::apply_operation(
        &mut review,
        &PaintReviewOperation::Forget {
            path: "a.paint".to_owned(),
        },
    );
    assert_eq!(review.approved().len(), 1);
}
