use beui_macros::{component, view};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, Prop};
use crate::styled::choice::{self, ChoiceOptionBuilder, Kind};
use crate::unstyled::ChoiceBuilder;

#[component]
pub fn tabs(labels: Vec<String>, selected: Prop<usize>, on_change: Callback<usize>) -> NodeId {
    let option_count = labels.len();
    let selected = selected.map(move |selected| Some(selected.min(option_count.saturating_sub(1))));
    view! {
        <choice
            labels
            selected
            kind=Kind::Tabs
            on_change={move |selected: Option<usize>| {
                if let Some(selected) = selected {
                    on_change.call(selected);
                }
            }}
        >
            {|handle| view! { <choice_option kind=Kind::Tabs handle /> }}
        </choice>
    }
}

pub fn tabs_selected(document: &Document, tabs: NodeId) -> usize {
    choice::selected_index(document, tabs).unwrap_or(0)
}
