use beui_macros::{component, view};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, Prop};
use crate::styled::choice::{self, ChoiceOption, Kind};
use crate::unstyled::Choice;

#[component]
pub fn Tabs(labels: Vec<String>, selected: Prop<usize>, on_change: Callback<usize>) -> NodeId {
    let option_count = labels.len();
    let selected = selected.map(move |selected| Some(selected.min(option_count.saturating_sub(1))));
    view! {
        <Choice
            labels
            selected
            kind=Kind::Tabs
            on_change={move |selected: Option<usize>| {
                if let Some(selected) = selected {
                    on_change.call(selected);
                }
            }}
        >
            {|handle| view! { <ChoiceOption kind=Kind::Tabs handle /> }}
        </Choice>
    }
}

pub fn tabs_selected(document: &Document, tabs: NodeId) -> usize {
    choice::selected_index(document, tabs).unwrap_or(0)
}
