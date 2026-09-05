use block::Block;
use uuid::Uuid;

use crate::{BlockClient, BlockHandleAccess};

#[cfg(test)]
mod tests;

macro_rules! block_types {
    ($($module:ident::$block:ident;)*) => {
        $(pub mod $module;)*

        pub const TYPE_IDS: &[Uuid] = &[$(<$module::$block as Block>::TYPE_ID,)*];

        pub fn open(
            client: &BlockClient,
            id: Uuid,
            block_type: Uuid,
        ) -> Option<Box<dyn BlockHandleAccess>> {
            $(
                if block_type == <$module::$block as Block>::TYPE_ID {
                    return Some(Box::new(client.get_block::<$module::$block>(id)));
                }
            )*
            None
        }
    };
}

block_types! {
    audio::Audio;
    calendar::Calendar;
    checklist::Checklist;
    compiled_logic::CompiledLogic;
    counter::Counter;
    database::Database;
    database_schema::DatabaseSchema;
    database_view::DatabaseView;
    deterministic_game::DeterministicGame;
    file_tree::FileTree;
    game_module::GameModule;
    gui_builder::GuiBuilder;
    hotbar::Hotbar;
    image::Image;
    infinite_canvas::InfiniteCanvas;
    logic_game::LogicGame;
    logic_grid::LogicGrid;
    map::Map;
    paint_review::PaintReview;
    paint_snapshot::PaintSnapshot;
    pdf::Pdf;
    pixel_art::PixelArt;
    pixel_ray_tracer::PixelRayTracer;
    presentation::Presentation;
    scene_3d::Scene3D;
    settings::Settings;
    text::TextDocument;
    ui_settings::UiSettings;
    version_control_data::VersionControlData;
    version_control_object::VersionControlObject;
    version_control_worktree::VersionControlWorktree;
    video::Video;
    web_browser_tab::WebBrowserTab;
    workspace_index::WorkspaceIndex;
}
