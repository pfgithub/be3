use super::*;

#[test]
fn serialization_round_trip() {
    let mut review = PaintReview::new();
    PaintReview::apply_operation(
        &mut review,
        &PaintReviewOperation::Approve {
            painting: painting("b.paint", "22"),
        },
    );
    PaintReview::apply_operation(
        &mut review,
        &PaintReviewOperation::Approve {
            painting: painting("a.paint", "11"),
        },
    );
    let encoded = serde_json::to_vec(&review).unwrap();
    assert_eq!(
        serde_json::from_slice::<PaintReview>(&encoded).unwrap(),
        review
    );
    let paths: Vec<&str> = review
        .approved()
        .iter()
        .map(|approved| approved.path.as_str())
        .collect();
    assert_eq!(paths, ["a.paint", "b.paint"]);
}

fn painting(path: &str, hash: &str) -> ApprovedPainting {
    ApprovedPainting {
        path: path.to_owned(),
        hash: hash.to_owned(),
        snapshot: BlockRef::Direct(Uuid::new_v4()),
    }
}
