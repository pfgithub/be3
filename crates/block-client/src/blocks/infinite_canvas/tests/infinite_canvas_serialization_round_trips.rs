use std::collections::BTreeMap;

use block::Block;
use uuid::Uuid;

use super::{
    CanvasComponent, CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint,
    CanvasTransform, InfiniteCanvas, InfiniteCanvasOperation,
};
use crate::block_ref::BlockRef;
use crate::blocks::database::DatabaseValue;

#[test]
fn infinite_canvas_serialization_round_trips() {
    let [schema_id, string_field, number_field, enum_field, option_id] =
        std::array::from_fn(|_| Uuid::new_v4());
    let values = BTreeMap::from([
        (
            string_field,
            DatabaseValue::String("component value".to_owned()),
        ),
        (number_field, DatabaseValue::Number(12.5)),
        (enum_field, DatabaseValue::Enum(option_id)),
    ]);
    let entity = CanvasEntity {
        id: Uuid::new_v4(),
        transform: CanvasTransform::new(
            CanvasPoint::new(10.0, -4.0),
            CanvasPoint::new(120.0, 80.0),
            0.25,
        ),
        kind: CanvasEntityKind::Rectangle,
        style: CanvasEntityStyle::default(),
        group_id: Some(Uuid::new_v4()),
        locked: true,
        components: vec![CanvasComponent {
            schema_id: BlockRef::Direct(schema_id),
            values,
        }],
    };
    let mut canvas = InfiniteCanvas::new();
    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::Add {
            entity: entity.clone(),
        },
    );
    let operation = InfiniteCanvasOperation::Update {
        entities: vec![entity],
    };

    let canvas_json = serde_json::to_vec(&canvas).unwrap();
    let operation_json = serde_json::to_vec(&operation).unwrap();

    assert_eq!(
        serde_json::from_slice::<InfiniteCanvas>(&canvas_json).unwrap(),
        canvas
    );
    assert_eq!(
        serde_json::from_slice::<InfiniteCanvasOperation>(&operation_json).unwrap(),
        operation
    );
}
