use super::*;

fn hello() -> Message {
    Message::Hello(Hello {
        minimum_version: PROTOCOL_VERSION,
        maximum_version: PROTOCOL_VERSION,
        plugin: PluginIdentity {
            id: "demo".into(),
            name: "Plugin Demo".into(),
            version: "1.0".into(),
        },
        capabilities: vec![
            Capability::Input,
            Capability::Surface(SurfaceMechanism::WebExternalImage),
        ],
    })
}

fn screen(
    screen: u64,
    instance: u64,
    pixel_width: u32,
    pixel_height: u32,
    scale_factor: f32,
) -> ScreenRequest {
    region_screen(
        EditorRegion::Main,
        screen,
        instance,
        pixel_width,
        pixel_height,
        scale_factor,
    )
}

fn region_screen(
    region: EditorRegion,
    screen: u64,
    instance: u64,
    pixel_width: u32,
    pixel_height: u32,
    scale_factor: f32,
) -> ScreenRequest {
    ScreenRequest {
        screen: ScreenId(screen),
        instance: EditorInstanceId(instance),
        region,
        metrics: ViewportMetrics {
            logical_width: pixel_width as f32 / scale_factor,
            logical_height: pixel_height as f32 / scale_factor,
            visible_x: 0.0,
            visible_y: 0.0,
            pixel_width,
            pixel_height,
            scale_factor,
        },
    }
}

mod artifact_messages_round_trip;
mod block_types_round_trip;
mod creation_messages_round_trip;
mod cursor_round_trips;
mod drag_messages_round_trip;
mod file_pick_messages_round_trip;
mod frame_round_trips;
mod manifest_validation;
mod multiplexed_messages_round_trip;
mod open_block_request_round_trips;
mod performance_messages_round_trip;
mod region_sizes_round_trip;
mod rejects_artifact_settings_over_limit;
mod rejects_collection_over_limit;
mod rejects_malformed_payload;
mod rejects_truncated_frame;
mod rejects_unknown_message_kind;
mod stacked_layout_keeps_each_region;
mod stacked_layout_stacks_screens;
mod view_messages_round_trip;
mod zoom_gesture_round_trips;
