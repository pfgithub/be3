use std::collections::BTreeMap;

use uuid::Uuid;

use super::{
    rebase_entity, CanvasColor, CanvasComponent, CanvasEntity, CanvasEntityKind, CanvasEntityStyle,
    CanvasPoint, CanvasTransform,
};
use crate::block_ref::BlockRef;
use crate::blocks::database::DatabaseValue;

fn entity() -> CanvasEntity {
    CanvasEntity {
        id: Uuid::new_v4(),
        transform: CanvasTransform::new(
            CanvasPoint::new(1.0, 2.0),
            CanvasPoint::new(10.0, 20.0),
            0.0,
        ),
        kind: CanvasEntityKind::Rectangle,
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
        components: Vec::new(),
    }
}

#[test]
fn rebase_entity_preserves_conflicting_remote_fields() {
    let [schema_id, remote_schema_id, local_field, remote_field] =
        std::array::from_fn(|_| Uuid::new_v4());
    let mut before = entity();
    before.components.push(CanvasComponent {
        schema_id: BlockRef::Direct(schema_id),
        values: BTreeMap::from([
            (
                local_field,
                DatabaseValue::String("local before".to_owned()),
            ),
            (
                remote_field,
                DatabaseValue::String("remote before".to_owned()),
            ),
        ]),
    });
    let mut after = before.clone();
    after.transform.center.x = 5.0;
    after.style.opacity = 0.5;
    after.components[0].values.insert(
        local_field,
        DatabaseValue::String("local after".to_owned()),
    );
    let mut remote = after.clone();
    remote.transform.center.x = 8.0;
    remote.style.foreground = CanvasColor::Rgba {
        red: 1,
        green: 2,
        blue: 3,
        alpha: 255,
    };
    remote.components[0].values.insert(
        remote_field,
        DatabaseValue::String("remote after".to_owned()),
    );
    remote.components.push(CanvasComponent {
        schema_id: BlockRef::Direct(remote_schema_id),
        values: BTreeMap::new(),
    });

    let undone = rebase_entity(&remote, &after, &before);
    assert_eq!(undone.transform.center.x, 8.0);
    assert_eq!(undone.style.opacity, 1.0);
    assert_eq!(undone.style.foreground, remote.style.foreground);
    assert_eq!(
        undone.components[0].values.get(&local_field),
        Some(&DatabaseValue::String("local before".to_owned()))
    );
    assert_eq!(
        undone.components[0].values.get(&remote_field),
        Some(&DatabaseValue::String("remote after".to_owned()))
    );
    assert_eq!(
        undone.components[1].schema_id,
        BlockRef::Direct(remote_schema_id)
    );

    let redone = rebase_entity(&undone, &before, &after);
    assert_eq!(
        redone.components[0].values.get(&local_field),
        Some(&DatabaseValue::String("local after".to_owned()))
    );
    assert_eq!(
        redone.components[0].values.get(&remote_field),
        Some(&DatabaseValue::String("remote after".to_owned()))
    );
    assert_eq!(
        redone.components[1].schema_id,
        BlockRef::Direct(remote_schema_id)
    );
}
