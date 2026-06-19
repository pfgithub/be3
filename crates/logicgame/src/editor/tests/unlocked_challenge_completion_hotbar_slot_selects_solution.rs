use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use super::*;

static TEST_ID: AtomicU64 = AtomicU64::new(0);

fn test_root() -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "logicgame-editor-components-{}-{}",
            std::process::id(),
            TEST_ID.fetch_add(1, Ordering::Relaxed)
        ))
        .join("components")
}

fn remove_test_root(root: &Path) {
    fs::remove_dir_all(root.parent().expect("test root has a parent")).unwrap();
}

#[test]
fn unlocked_challenge_completion_hotbar_slot_selects_solution() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());
    let (id, name, mut grid) = files.create_challenge_solution(ChallengeId::And).unwrap();
    let file = ComponentFileRef { id };
    grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::MergerSplitter {
            input_scale: scale(8),
            output_scale: scale(8),
        },
    );
    grid.add_component(
        Point::new(0, -1),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(0),
            label: String::new(),
        },
    );
    grid.add_component(
        Point::new(4, 8),
        Rotation::Down,
        ComponentKind::Output {
            scale: Scale::ONE,
            id: OutputId::from_u128(0),
            label: String::new(),
        },
    );
    files
        .save_challenge_solution(ChallengeId::And, id, &grid, true)
        .unwrap();

    let mut editor = LogicEditor::default();
    editor.set_component_files(Some(files));

    editor.select_hotbar_path(vec![3, 1]);

    assert_eq!(editor.tool.kind, ToolKind::Custom);
    assert_eq!(
        editor.hotbar_slot_label(get_hotbar_slot(&editor.hotbar, &[3, 1]).unwrap()),
        name
    );
    let Some(ComponentKind::Subcomponent {
        name: component_name,
        source_file_id,
        ..
    }) = editor.selected_custom_kind()
    else {
        panic!("unlocked challenge slot should resolve to a subcomponent");
    };
    assert_eq!(component_name, name);
    assert_eq!(source_file_id, file);

    remove_test_root(&root);
}
