use block::Block;
use uuid::Uuid;

use super::{
    GuiBuilder, GuiBuilderOperation, GuiCanvasSize, GuiLayout, GuiLocation, GuiWidget,
    GuiWidgetKind, MIN_CANVAS_SIZE,
};
use crate::BlockClient;

mod canvas_size_is_clamped_to_the_supported_range;
mod gui_builder_history_undoes_and_redoes_changes;
mod gui_builder_serialization_round_trips;
mod inserting_into_a_non_container_is_ignored;
mod moving_a_container_into_itself_is_ignored;
mod removing_a_container_removes_its_children;
mod widgets_are_inserted_at_their_location;

fn container() -> GuiWidget {
    GuiWidget::new(GuiWidgetKind::Container {
        layout: GuiLayout::Vertical,
        framed: false,
    })
}

fn label(text: &str) -> GuiWidget {
    GuiWidget::new(GuiWidgetKind::Label { text: text.into() })
}

fn insert(builder: &mut GuiBuilder, parent: Option<Uuid>, index: usize, widget: GuiWidget) -> Uuid {
    let id = widget.id;
    GuiBuilder::apply_operation(
        builder,
        &GuiBuilderOperation::Insert {
            location: GuiLocation::new(parent, index),
            widget,
        },
    );
    id
}
