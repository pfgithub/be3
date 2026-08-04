use block::Block;
use uuid::Uuid;

use crate::blocks::gui_builder::{
    GuiBuilder, GuiBuilderOperation, GuiLayout, GuiLocation, GuiWidget, GuiWidgetKind,
};

mod generated_code_binds_stateful_widgets_to_struct_fields;
mod generated_code_nests_container_children;
mod repeated_labels_get_unique_field_names;
mod struct_name_override_replaces_the_title;

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

fn push(builder: &mut GuiBuilder, kind: GuiWidgetKind) -> Uuid {
    let index = builder.widgets().len();
    insert(builder, None, index, GuiWidget::new(kind))
}
