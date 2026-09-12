use beui_macros::{component, view};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, Prop};
use crate::styled::choice::{self, ChoiceOption, Kind};
use crate::unstyled::Choice;

#[component]
pub fn RadioGroup(
    labels: Vec<String>,
    selected: Prop<Option<usize>>,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    view! {
        <Choice
            labels
            selected
            kind=Kind::Radio
            on_change={move |selected| on_change.call(selected)}
        >
            {|handle| view! { <ChoiceOption kind=Kind::Radio handle /> }}
        </Choice>
    }
}

pub fn radio_group_selected(document: &Document, control: NodeId) -> Option<usize> {
    choice::selected_index(document, control)
}
