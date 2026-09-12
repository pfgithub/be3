use super::*;
use crate::reactive::{
    build, create_memo, create_signal, view, ButtonBuilder, ColumnBuilder, ItemSize, NodeRef,
    RowBuilder, TextBuilder,
};

const FIXED_WIDTH: f32 = 30.0;

#[test]
fn a_reactive_sizing_attribute_moves_a_child_between_fixed_and_percent() {
    let (toggle, left, right) = (NodeRef::new(), NodeRef::new(), NodeRef::new());

    let document = build({
        let (toggle, left, right) = (toggle.clone(), left.clone(), right.clone());
        move || {
            let (shared, set_shared) = create_signal(false);
            let sizing = create_memo(move || {
                if shared.get() {
                    ItemSize::Percent(100.0)
                } else {
                    ItemSize::Fixed(FIXED_WIDTH)
                }
            });
            view! {
                <column spacing=0.0>
                    <button
                        @node_ref=&toggle
                        on_click={move || set_shared.update(|shared| *shared = !*shared)}
                    >
                        <text string="toggle" />
                    </button>
                    <row @sizing=ItemSize::Percent(100.0) spacing=0.0>
                        <column @sizing={sizing} @node_ref=&left spacing=0.0></column>
                        <column @sizing=ItemSize::Percent(100.0) @node_ref=&right spacing=0.0></column>
                    </row>
                </column>
            }
        }
    });

    let (toggle, left, right) = (toggle.get(), left.get(), right.get());
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(left).width(), FIXED_WIDTH);
    assert_eq!(harness.rect(right).width(), WIDE_VIEWPORT.x - FIXED_WIDTH);

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(left).width(),
        WIDE_VIEWPORT.x / 2.0,
        "a memo behind `@sizing` must relay a new `ItemSize` to the row that lays the child out"
    );
    assert_eq!(harness.rect(right).width(), WIDE_VIEWPORT.x / 2.0);
}
