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
        capabilities: vec![Capability::Input, Capability::Surface],
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
        EditorRegion::Frame,
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
        frame: None,
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
mod audio_messages_round_trip;
mod block_types_round_trip;
mod child_placements_round_trip;
mod child_statuses_round_trip;
mod child_view_changes_round_trip;
mod clipboard_messages_round_trip;
mod copied_text_round_trips;
mod creation_messages_round_trip;
mod cursor_round_trips;
mod drag_messages_round_trip;
mod every_editor_manifest_parses;
mod fetch_messages_round_trip;
mod file_drop_messages_round_trip;
mod file_pick_messages_round_trip;
mod frame_round_trips;
mod frame_screens_and_reports_round_trip;
mod grabbing_the_cursor_round_trips;
mod ime_messages_round_trip;
mod manifest_validation;
mod multiplexed_messages_round_trip;
mod open_block_request_round_trips;
mod open_messages_round_trip;
mod packed_layout_keeps_each_region;
mod packed_layout_packs_screens_within_a_row;
mod performance_messages_round_trip;
mod pick_block_messages_round_trip;
mod presence_messages_round_trip;
mod present_messages_round_trip;
mod region_sizes_round_trip;
mod rejects_artifact_settings_over_limit;
mod rejects_collection_over_limit;
mod rejects_malformed_payload;
mod rejects_truncated_frame;
mod rejects_unknown_message_kind;
mod rejects_unordered_occluders;
mod replacing_a_child_round_trips;
mod resize_messages_round_trip;
mod view_messages_round_trip;
mod web_view_messages_round_trip;
mod zoom_gesture_round_trips;
