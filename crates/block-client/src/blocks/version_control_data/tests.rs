use block::Block;
use uuid::Uuid;

use super::*;

fn apply(data: &mut VersionControlData, operation: VersionControlDataOperation) {
    VersionControlData::apply_operation(data, &operation);
}

fn author() -> Uuid {
    Uuid::from_u128(0xa1)
}

mod version_control_data_ancestors_walks_the_chain;
mod version_control_data_append_commit_adds_to_history;
mod version_control_data_append_commit_ignores_dangling_parent;
mod version_control_data_new_seeds_main_with_empty_commit;
mod version_control_data_references_registered_objects;
mod version_control_data_register_object_first_writer_wins;
mod version_control_data_set_branch_accepted_when_unchanged;
mod version_control_data_set_branch_creates_new_branch;
mod version_control_data_set_branch_ignores_unknown_commit;
mod version_control_data_set_branch_rejected_when_moved;
