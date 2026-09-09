use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{with_document, Children, FillBuilder, OutlineBuilder};
use crate::styled::theme::{BORDER, BORDER_WIDTH};

#[component]
pub fn bordered(corner_radius: u8, children: Children) -> NodeId {
    let child = children
        .into_first()
        .expect("bordered requires a child, e.g. <bordered>{content}</bordered>");
    view! {
        <outline color={BORDER} width={BORDER_WIDTH} radius={corner_radius} offset={0.0} visible={true}>
            {child}
        </outline>
    }
}

#[component]
pub fn separator() -> NodeId {
    let fill = FillBuilder::default()
        .color(BORDER)
        .radius(0)
        .children([])
        .build();
    with_document(|document| document.create_shadow("separator", fill, Vec::new()))
}
