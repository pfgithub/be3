use super::*;

#[test]
fn a_new_scene_paints_nothing_until_the_lighting_lands() {
    let (mut editor, block, _host) = editor();

    let scene = block.read().unwrap();
    assert!(scene
        .pixels()
        .iter()
        .all(|pixel| *pixel == PIXEL_RAY_TRACER_BACKGROUND));
    assert!(scene.entities().is_empty());
    drop(scene);
    editor.snapshot("a_new_scene_paints_nothing_until_the_lighting_lands");
}
