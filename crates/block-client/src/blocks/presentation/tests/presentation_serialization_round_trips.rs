use uuid::Uuid;

use super::{Presentation, PresentationOperation, PresentationSlide};
use crate::block_ref::BlockRef;
use block::Block;

#[test]
fn presentation_serialization_round_trips() {
    let slide = PresentationSlide {
        id: Uuid::new_v4(),
        block_id: BlockRef::Direct(Uuid::new_v4()),
    };
    let mut presentation = Presentation::new();
    Presentation::apply_operation(
        &mut presentation,
        &PresentationOperation::Insert { slide, index: 0 },
    );
    let json = serde_json::to_string(&presentation).unwrap();
    assert_eq!(
        serde_json::from_str::<Presentation>(&json).unwrap(),
        presentation
    );
}
