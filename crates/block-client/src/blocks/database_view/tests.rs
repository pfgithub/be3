use block::Block;
use uuid::Uuid;

use super::{
    DatabaseView, DatabaseViewKind, DatabaseViewOperation, DatabaseViewSort, SortDirection,
};

mod database_view_references_its_database;
mod set_kanban_field_changes_the_kanban_field;
mod set_kind_changes_the_view_kind;
mod set_scatter_x_field_changes_the_scatter_x_field;
mod set_scatter_y_field_changes_the_scatter_y_field;
mod set_sort_replaces_or_clears_the_sort;
