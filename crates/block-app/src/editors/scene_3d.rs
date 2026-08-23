mod camera;
pub(super) mod renderer;

use block_client::{blocks::scene_3d::Scene3D, BlockClient, BlockHandle};
use eframe::egui;
use egui_material_icons::{icons::ICON_VIEW_IN_AR, MaterialIcon};

use self::{
    camera::Camera,
    renderer::{Scene3DCallback, SceneFrame},
};
use super::{
    BlockEditor, CreatableEditor, DirectEditorCapabilities, DirectEditorViewport,
    DirectEditorViewportInput, EditorAccess, EditorAction, EditorKind,
};

const HINT_TEXT: &str = "Click to look around \u{2022} WASD to move \u{2022} Esc to release";

impl EditorKind for Scene3DEditor {
    type Block = Scene3D;

    const DISPLAY_NAME: &'static str = "3D scene";
    const ICON: MaterialIcon = ICON_VIEW_IN_AR;

    fn open(_client: &BlockClient, block: BlockHandle<Scene3D>) -> Self {
        Self::new(block)
    }
}

impl CreatableEditor for Scene3DEditor {
    fn create(client: &BlockClient) -> Self {
        Self::new(client.create_block(Scene3D::new()))
    }
}

pub(super) struct Scene3DEditor {
    block: BlockHandle<Scene3D>,
    camera: Camera,
    focused: bool,
}

impl Scene3DEditor {
    fn new(block: BlockHandle<Scene3D>) -> Self {
        Self {
            block,
            camera: Camera::default(),
            focused: false,
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        if !self.focused {
            if response.clicked() {
                self.set_focused(ctx, true);
            }
            return;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.set_focused(ctx, false);
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

    fn set_focused(&mut self, ctx: &egui::Context, focused: bool) {
        self.focused = focused;
        let grab = if focused {
            egui::CursorGrab::Locked
        } else {
            egui::CursorGrab::None
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::CursorGrab(grab));
        ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(!focused));
    }
}

impl BlockEditor for Scene3DEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: true,
        }
    }

    fn direct_editor_fills_viewport(&self) -> bool {
        true
    }

    fn direct_editor_viewport_input(
        &self,
        _editors: &EditorAccess<'_>,
    ) -> DirectEditorViewportInput {
        DirectEditorViewportInput::Editor
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        self.handle_input(ui.ctx(), &response);

        let size = response.rect.size().max(egui::vec2(1.0, 1.0));
        let pixels_per_point = ui.ctx().pixels_per_point();
        let frame = SceneFrame {
            viewport_size_px: [
                (size.x * pixels_per_point).round() as u32,
                (size.y * pixels_per_point).round() as u32,
            ],
            view_projection: self.camera.view_projection(size.x / size.y),
        };
        painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
            response.rect,
            Scene3DCallback { frame },
        ));

        if !self.focused {
            painter.text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                HINT_TEXT,
                egui::FontId::proportional(15.0),
                egui::Color32::from_white_alpha(230),
            );
        }

        None
    }
}
