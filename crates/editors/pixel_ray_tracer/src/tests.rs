use std::sync::Arc;

use block_client::blocks::pixel_ray_tracer::{
    PixelRayTracer, PixelRayTracerOperation, PixelUpdate, PIXEL_RAY_TRACER_BACKGROUND,
};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::PixelRayTracerApp;

mod a_new_scene_paints_nothing_until_the_lighting_lands;
mod resetting_the_artwork_clears_painted_pixels;
mod zooming_the_view_grows_the_scene;

fn editor() -> (
    EditorTest<'static, PixelRayTracerApp>,
    BlockHandle<PixelRayTracer>,
    EditorHost,
) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(PixelRayTracer::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = PixelRayTracerApp::default();
    app.connect(host.clone(), client, block.id());
    let mut editor = EditorTest::viewport(app, host.clone());
    editor.step();
    (editor, block, host)
}
