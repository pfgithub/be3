use block::Block;
use uuid::Uuid;

use crate::{BlockClient, BlockHandleAccess};

#[cfg(test)]
mod tests;

macro_rules! block_types {
    ($($module:ident::$block:ident $(: $default:path)?;)*) => {
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

        pub fn create_default(
            client: &BlockClient,
            block_type: Uuid,
        ) -> Option<Box<dyn BlockHandleAccess>> {
            $($(
                if block_type == <$module::$block as Block>::TYPE_ID {
                    let value = <$module::$block as $default>::default();
                    return Some(Box::new(client.create_block(value)));
                }
            )?)*
            None
        }
    };
}

block_types! {
    audio::Audio;
    calendar::Calendar: Default;
    checklist::Checklist: Default;
    compiled_logic::CompiledLogic;
    counter::Counter: Default;
    database::Database;
    database_schema::DatabaseSchema: Default;
    database_view::DatabaseView;
    deterministic_game::DeterministicGame: Default;
    gui_builder::GuiBuilder: Default;
    hotbar::Hotbar: Default;
    image::Image;
    infinite_canvas::InfiniteCanvas: Default;
    logic_game::LogicGame: Default;
    logic_grid::LogicGrid: Default;
    map::Map: Default;
    pdf::Pdf;
    pixel_art::PixelArt: Default;
    pixel_ray_tracer::PixelRayTracer: Default;
    presentation::Presentation: Default;
    scene_3d::Scene3D: Default;
    settings::Settings: Default;
    text::TextDocument: Default;
    ui_settings::UiSettings: Default;
    version_control_data::VersionControlData;
    version_control_object::VersionControlObject;
    version_control_worktree::VersionControlWorktree;
    video::Video: Default;
    web_browser_tab::WebBrowserTab: Default;
    workspace_index::WorkspaceIndex: Default;
}
