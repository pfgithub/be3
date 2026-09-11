use beui_macros::{component, view};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    create_memo, create_signal, set_component_state, Callback, ColumnBuilder, Prop, ReadSignal,
    ShowBuilder,
};
use crate::styled::theme::NARROW_WIDTH;
use crate::styled::{SelectBuilder, TabsBuilder};
use crate::unstyled::narrower_than;

#[component]
pub fn responsive_tabs(
    labels: Vec<String>,
    selected: Prop<usize>,
    on_change: Callback<usize>,
    #[prop(default = NARROW_WIDTH)] breakpoint: f32,
) -> NodeId {
    let (selected_read, set_selected) = create_signal(0usize);
    selected.apply({
        let set_selected = set_selected.clone();
        move |value| set_selected.set(value)
    });
    set_component_state(selected_read.clone());

    let narrow = narrower_than(breakpoint);
    let wide = create_memo({
        let narrow = narrow.clone();
        move || !narrow.get()
    });
    let highlighted = create_memo({
        let selected_read = selected_read.clone();
        move || Some(selected_read.get())
    });

    let tab_labels = labels.clone();
    let tab_selected = selected_read.clone();
    let tab_change = on_change.clone();
    let tab_set = set_selected.clone();

    view! {
        <column spacing={0.0}>
            <show condition={wide} then={move || view! {
                <tabs labels={tab_labels} selected={tab_selected} on_change={move |index| {
                    tab_set.set(index);
                    tab_change.call(index);
                }} />
            }} />
            <show condition={narrow} then={move || view! {
                <select options={labels} selected={highlighted} on_change={move |index: Option<usize>| {
                    if let Some(index) = index {
                        set_selected.set(index);
                        on_change.call(index);
                    }
                }} />
            }} />
        </column>
    }
}

pub fn responsive_tabs_selected(document: &Document, tabs: NodeId) -> usize {
    document.component_state::<ReadSignal<usize>>(tabs).get()
}
