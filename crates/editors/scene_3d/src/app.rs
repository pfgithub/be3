use std::sync::Arc;

use block_client::blocks::scene_3d::Scene3D;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::{egui, EditorHost};
use uuid::Uuid;

use crate::camera::Camera;
use crate::scene::{self, SceneFrame};

const HINT_TEXT: &str = "Click to look around \u{2022} WASD to move \u{2022} Esc to release";

#[derive(Default)]
pub struct Scene3DApp {
    host: Option<EditorHost>,
    creation: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<Scene3D>>,
    camera: Camera,
}

impl Scene3DApp {
    fn looking(&self) -> bool {
        self.host.as_ref().is_some_and(EditorHost::cursor_grabbed)
    }

    fn set_looking(&self, looking: bool) {
        if let Some(host) = self.host.as_ref() {
            host.grab_cursor(looking);
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        if !self.looking() {
            if response.clicked() {
                self.set_looking(true);
            }
            return;
        }
        let (escaped, focused) =
            ctx.input(|input| (input.key_pressed(egui::Key::Escape), input.focused));
        if escaped || !focused {
            self.set_looking(false);
            return;
        }

        let (motion, forward, back, left, right, dt) = ctx.input(|input| {
            (
                input.pointer.motion().unwrap_or_default(),
                input.key_down(egui::Key::W) || input.key_down(egui::Key::ArrowUp),
                input.key_down(egui::Key::S) || input.key_down(egui::Key::ArrowDown),
                input.key_down(egui::Key::A) || input.key_down(egui::Key::ArrowLeft),
                input.key_down(egui::Key::D) || input.key_down(egui::Key::ArrowRight),
                input.stable_dt,
            )
        });
        self.camera.look([motion.x, motion.y]);
        self.camera.walk(
            (right as i32 - left as i32) as f32,
            (forward as i32 - back as i32) as f32,
            dt,
        );
        ctx.request_repaint();
    }
}

impl block_editor_plugin::App for Scene3DApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.block = Some(client.get_block(block_id));
        self.host = Some(host);
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(Scene3D::new()).id())
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let response = response.test_id("scene.viewport");
        self.handle_input(ui.ctx(), &response);

        let size = response.rect.size().max(egui::vec2(1.0, 1.0));
        let pixels_per_point = ui.ctx().pixels_per_point();
        scene::paint(
            &painter,
            response.rect,
            SceneFrame {
                viewport_size_px: [
                    (size.x * pixels_per_point).round() as u32,
                    (size.y * pixels_per_point).round() as u32,
                ],
                view_projection: self.camera.view_projection(size.x / size.y),
            },
        );

        if !self.looking() {
            painter.text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                HINT_TEXT,
                egui::FontId::proportional(15.0),
                egui::Color32::from_white_alpha(230),
            );
        }
    }
}
