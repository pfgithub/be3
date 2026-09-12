use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{
    clone, component_accessibility, component_detail, create_memo, Fill, ItemSize, Prop, Row,
    Sized, Spacer,
};
use crate::styled::theme::{ACCENT, TRACK};

const HEIGHT: f32 = 6.0;
const RADIUS: u8 = 3;

#[component]
pub fn Progress(value: Prop<f32>, #[prop(default = String::new())] label: Prop<String>) -> NodeId {
    let value = value.map(|value| value.clamp(0.0, 1.0));
    let value_read = create_memo(move || value.get());

    let filled = create_memo(clone!(value_read -> move || filled_size(value_read.get())));
    let rest = create_memo(clone!(value_read -> move || rest_size(value_read.get())));
    component_detail(create_memo(
        clone!(value_read -> move || detail(value_read.get())),
    ));
    component_accessibility(create_memo(clone!(value_read -> move || {
        let value = value_read.get();
        let mut node = Node::new(Role::ProgressIndicator);
        let label = label.get();
        if !label.is_empty() {
            node.set_label(label);
        }
        node.set_numeric_value(value.into());
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(1.0);
        node
    })));

    view! {
        <Sized height=HEIGHT>
            <Fill color=TRACK radius=RADIUS>
                <Row spacing=0.0>
                    <Fill @sizing={filled} color=ACCENT radius=RADIUS></Fill>
                    <Spacer @sizing={rest} />
                </Row>
            </Fill>
        </Sized>
    }
}

fn detail(value: f32) -> String {
    format!("{}%", (value * 100.0).round())
}

fn filled_size(value: f32) -> ItemSize {
    ItemSize::Percent(value * 100.0)
}

fn rest_size(value: f32) -> ItemSize {
    ItemSize::Percent((1.0 - value) * 100.0)
}
