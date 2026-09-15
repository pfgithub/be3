use super::*;
use crate::reactive::{WriteSignal, create_memo, create_signal, with_reactive_scope};

const SHORT: f32 = 60.0;
const TALL: f32 = 120.0;
const BOTTOM: f32 = 40.0;

#[test]
fn resizing_an_element_damages_where_it_was_and_where_it_moved_to() {
    let bottom = NodeRef::new();
    let taken: Rc<Cell<Option<WriteSignal<bool>>>> = Rc::new(Cell::new(None));
    let sink = taken.clone();
    let document = build({
        let bottom = bottom.clone();
        move || {
            let (tall, set_tall) = create_signal(false);
            sink.set(Some(set_tall));
            let height = create_memo(move || if tall.get() { TALL } else { SHORT });
            view! {
                <Column spacing=0.0>
                    <Frame height={height} color=Color32::WHITE radius=0 />
                    <Frame
                        @node_ref=&bottom
                        height={BOTTOM}
                        color={Color32::from_gray(40)}
                        radius=0
                    />
                </Column>
            }
        }
    });
    let bottom = bottom.get();
    let set_tall = taken.take().expect("the view published its signal");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let was = harness.rect(bottom);

    with_reactive_scope(harness.document_mut(), move || set_tall.set(true));
    let damage = harness
        .frame(Vec::new())
        .damage()
        .expect("resizing repaints");
    let moved = harness.rect(bottom);

    assert_eq!(was.top(), SHORT);
    assert_eq!(moved.top(), TALL);
    assert_eq!(
        damage,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(VIEWPORT.x, TALL + BOTTOM))
    );
}
