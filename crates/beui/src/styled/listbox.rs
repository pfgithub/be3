use beui_macros::{component, view};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, Prop};
use crate::styled::choice::{self, ChoiceOptionBuilder, Kind};
use crate::unstyled::ChoiceBuilder;

#[component]
pub fn listbox(
    labels: Vec<String>,
    selected: Prop<Option<usize>>,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    view! {
        <choice
            labels
            selected
            kind=Kind::Listbox
            on_change={move |selected| on_change.call(selected)}
            option={|handle| view! { <choice_option kind=Kind::Listbox handle /> }}
        />
    }
}

pub fn listbox_selected(document: &Document, control: NodeId) -> Option<usize> {
    choice::selected_index(document, control)
}
