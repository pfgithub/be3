use beui_macros::{component, view};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    clone, create_effect, create_memo, create_signal, set_component_state, Callback, Column, Prop,
    ReadSignal, Show,
};
use crate::styled::theme::NARROW_WIDTH;
use crate::styled::{Select, Tabs};
use crate::unstyled::narrower_than;

#[component]
pub fn ResponsiveTabs(
    labels: Vec<String>,
    selected: Prop<usize>,
    on_change: Callback<usize>,
    #[prop(default = NARROW_WIDTH)] breakpoint: f32,
) -> NodeId {
    let (selected_read, set_selected) = create_signal(selected.peek());
    create_effect(clone!(set_selected -> move || set_selected.set(selected.get())));
    set_component_state(selected_read.clone());

    let narrow = narrower_than(breakpoint);
    let wide = create_memo(clone!(narrow -> move || !narrow.get()));
    let highlighted = create_memo(clone!(selected_read -> move || Some(selected_read.get())));

    let tab_labels = labels.clone();
    let tab_selected = selected_read.clone();
    let tab_change = on_change.clone();
    let tab_set = set_selected.clone();

    view! {
        <Column spacing=0.0>
            <Show condition={wide} then={move || view! {
                <Tabs labels={tab_labels} selected={tab_selected} on_change={move |index| {
                    tab_set.set(index);
                    tab_change.call(index);
                }} />
            }} />
            <Show condition={narrow} then={move || view! {
                <Select options={labels} selected={highlighted} on_change={move |index: Option<usize>| {
                    if let Some(index) = index {
                        set_selected.set(index);
                        on_change.call(index);
                    }
                }} />
            }} />
        </Column>
    }
}

pub fn responsive_tabs_selected(document: &Document, tabs: NodeId) -> usize {
    document.component_state::<ReadSignal<usize>>(tabs).get()
}
