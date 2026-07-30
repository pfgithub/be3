use block_client::blocks::infinite_canvas::{
    CanvasColor, CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint, CanvasTransform,
};
use uuid::Uuid;

use super::rebase_entity;

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
    }
}

#[test]
fn rebase_entity_preserves_conflicting_remote_fields() {
    let before = entity();
    let mut after = before.clone();
    after.transform.center.x = 5.0;
    after.style.opacity = 0.5;
    let mut remote = after.clone();
    remote.transform.center.x = 8.0;
    remote.style.foreground = CanvasColor::Rgba {
        red: 1,
        green: 2,
        blue: 3,
        alpha: 255,
    };

    let undone = rebase_entity(&remote, &after, &before);
    assert_eq!(undone.transform.center.x, 8.0);
    assert_eq!(undone.style.opacity, 1.0);
    assert_eq!(undone.style.foreground, remote.style.foreground);
}
