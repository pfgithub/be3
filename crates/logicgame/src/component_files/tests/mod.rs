use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use logicgame::{
    execution::Instruction,
    grid::{ComponentKind, ComponentSide, Point, Rotation, Scale},
};

use super::*;

static TEST_ID: AtomicU64 = AtomicU64::new(0);

fn test_root() -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "logicgame-components-{}-{}",
            std::process::id(),
            TEST_ID.fetch_add(1, Ordering::Relaxed)
        ))
        .join("components")
}

fn remove_test_root(root: &Path) {
    fs::remove_dir_all(root.parent().expect("test root has a parent")).unwrap();
}

mod accepts_non_boundary_subcomponents;
mod compiled_component_serializes_unlinked_component_and_source_id;
mod compiles_and_reuses_content_addressed_subcomponents;
mod compiles_challenge_solution_as_subcomponent;
mod creates_lists_saves_and_loads_challenge_solutions_from_save_index;
mod creates_lists_saves_and_loads_components_by_uuid;
mod rejects_empty_and_non_boundary_subcomponents;
mod rejects_invalid_and_duplicate_renames;
mod renames_only_save_index_metadata;
mod reports_malformed_component_files;
mod saves_challenge_progress_in_save_index_independently_from_solution_completion;
mod validates_component_names;
