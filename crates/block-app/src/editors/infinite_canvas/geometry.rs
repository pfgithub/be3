use super::*;

pub(super) fn duplicate_entities(
    entities: Vec<CanvasEntity>,
    offset: CanvasPoint,
) -> Vec<CanvasEntity> {
    let mut duplicate_groups = HashMap::new();
    entities
        .into_iter()
        .map(|mut entity| {
            entity.id = Uuid::new_v4();
            if let Some(group_id) = entity.group_id {
                entity.group_id = Some(
                    *duplicate_groups
                        .entry(group_id)
                        .or_insert_with(Uuid::new_v4),
                );
            }
            entity.transform.center.x += offset.x;
            entity.transform.center.y += offset.y;
            entity
        })
        .collect()
}

pub(super) fn focused_direct_editor(
    focused: Option<Uuid>,
    entities: &[CanvasEntity],
) -> Option<(Uuid, BlockRef, f32)> {
    let focused = focused?;
    entities.iter().find_map(|entity| match entity.kind {
        CanvasEntityKind::DirectEditor {
            block_id, scale, ..
        } if entity.id == focused => Some((entity.id, block_id, scale)),
        _ => None,
    })
}

pub(super) fn entity_kind_label(kind: &CanvasEntityKind) -> &'static str {
    match kind {
        CanvasEntityKind::Line => "Line",
        CanvasEntityKind::Rectangle => "Rectangle",
        CanvasEntityKind::Text { .. } => "Text",
        CanvasEntityKind::Pen { .. } => "Freehand",
        CanvasEntityKind::Block { .. } => "Block preview",
        CanvasEntityKind::DirectEditor { .. } => "Direct editor",
    }
}

pub(super) fn preview_region_for_entities(entities: &[CanvasEntity]) -> CanvasPreviewRegion {
    let bounds = entities.iter().map(entity_bounds).reduce(WorldRect::union);
    bounds.map_or_else(
        || CanvasPreviewRegion::new(CanvasPoint::default(), CanvasPoint::new(100.0, 100.0)),
        |bounds| {
            CanvasPreviewRegion::new(
                bounds.center(),
                CanvasPoint::new(bounds.size().x.max(100.0), bounds.size().y.max(100.0)),
            )
        },
    )
}

pub(super) fn preview_region_bounds(region: CanvasPreviewRegion) -> WorldRect {
    let half = CanvasPoint::new(region.size.x * 0.5, region.size.y * 0.5);
    WorldRect {
        min: CanvasPoint::new(region.center.x - half.x, region.center.y - half.y),
        max: CanvasPoint::new(region.center.x + half.x, region.center.y + half.y),
    }
}

pub(super) fn common_value<T: Copy + PartialEq>(
    values: impl IntoIterator<Item = T>,
) -> CommonValue<T> {
    let mut values = values.into_iter();
    let Some(first) = values.next() else {
        return CommonValue::None;
    };
    if values.all(|value| value == first) {
        CommonValue::Uniform(first)
    } else {
        CommonValue::Mixed
    }
}

pub(super) fn mixed_checkbox(
    ui: &mut egui::Ui,
    label: &str,
    value: CommonValue<bool>,
) -> Option<bool> {
    let mixed = matches!(value, CommonValue::Mixed);
    let mut checked = matches!(value, CommonValue::Uniform(true));
    let label = if mixed {
        format!("{label} (Mixed)")
    } else {
        label.into()
    };
    ui.checkbox(&mut checked, label)
        .changed()
        .then_some(checked)
}

const COLOR_PRESETS: [(&str, CanvasColor); 5] = [
    ("Default", CanvasColor::Auto),
    (
        "Red",
        CanvasColor::Rgba {
            red: 224,
            green: 49,
            blue: 49,
            alpha: 255,
        },
    ),
    (
        "Orange",
        CanvasColor::Rgba {
            red: 240,
            green: 140,
            blue: 0,
            alpha: 255,
        },
    ),
    (
        "Green",
        CanvasColor::Rgba {
            red: 47,
            green: 158,
            blue: 68,
            alpha: 255,
        },
    ),
    (
        "Blue",
        CanvasColor::Rgba {
            red: 25,
            green: 113,
            blue: 194,
            alpha: 255,
        },
    ),
];

pub(super) fn color_menu(
    ui: &mut egui::Ui,
    label: &str,
    value: CommonValue<CanvasColor>,
) -> Option<CanvasColor> {
    let mut changed = None;
    let current = match value {
        CommonValue::Uniform(color) => color,
        CommonValue::Mixed | CommonValue::None => CanvasColor::Auto,
    };
    ui.horizontal(|ui| {
        ui.label(label);
        for (name, color) in COLOR_PRESETS {
            if color_button(ui, name, color, value == CommonValue::Uniform(color)).clicked() {
                changed = Some(color);
            }
        }
        let mut color = resolve_color(current, ui.visuals().text_color()).to_srgba_unmultiplied();
        if ui
            .color_edit_button_srgba_unmultiplied(&mut color)
            .on_hover_text("Custom color")
            .changed()
        {
            changed = Some(CanvasColor::Rgba {
                red: color[0],
                green: color[1],
                blue: color[2],
                alpha: color[3],
            });
        }
    });
    changed
}

pub(super) fn fill_color_menu(
    ui: &mut egui::Ui,
    value: CommonValue<Option<CanvasColor>>,
) -> Option<Option<CanvasColor>> {
    let mut changed = None;
    let current = match value {
        CommonValue::Uniform(Some(color)) => color,
        CommonValue::Uniform(None) | CommonValue::Mixed | CommonValue::None => CanvasColor::Auto,
    };
    ui.horizontal(|ui| {
        ui.label("Fill");
        if ui
            .selectable_label(value == CommonValue::Uniform(None), ICON_FORMAT_COLOR_RESET)
            .on_hover_text("No fill")
            .clicked()
        {
            changed = Some(None);
        }
        for (name, color) in COLOR_PRESETS {
            if color_button(ui, name, color, value == CommonValue::Uniform(Some(color))).clicked() {
                changed = Some(Some(color));
            }
        }
        let mut color = resolve_color(current, ui.visuals().text_color()).to_srgba_unmultiplied();
        if ui
            .color_edit_button_srgba_unmultiplied(&mut color)
            .on_hover_text("Custom color")
            .changed()
        {
            changed = Some(Some(CanvasColor::Rgba {
                red: color[0],
                green: color[1],
                blue: color[2],
                alpha: color[3],
            }));
        }
    });
    changed
}

pub(super) fn color_button(
    ui: &mut egui::Ui,
    name: &str,
    color: CanvasColor,
    selected: bool,
) -> egui::Response {
    let color = resolve_color(color, ui.visuals().text_color());
    ui.selectable_label(selected, ICON_CIRCLE.rich_text().color(color))
        .on_hover_text(name)
}

pub(super) fn resolve_color(color: CanvasColor, auto: Color32) -> Color32 {
    match color {
        CanvasColor::Auto => auto,
        CanvasColor::Rgba {
            red,
            green,
            blue,
            alpha,
        } => Color32::from_rgba_unmultiplied(red, green, blue, alpha),
    }
}

pub(super) fn midpoint(a: CanvasPoint, b: CanvasPoint) -> CanvasPoint {
    CanvasPoint::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
}

pub(super) fn distance(a: CanvasPoint, b: CanvasPoint) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

pub(super) fn rotate(point: CanvasPoint, angle: f32) -> CanvasPoint {
    let (sin, cos) = angle.sin_cos();
    CanvasPoint::new(point.x * cos - point.y * sin, point.x * sin + point.y * cos)
}

pub(super) fn local_to_world(transform: CanvasTransform, local: CanvasPoint) -> CanvasPoint {
    let scaled = CanvasPoint::new(local.x * transform.size.x, local.y * transform.size.y);
    let rotated = rotate(scaled, transform.rotation);
    CanvasPoint::new(
        transform.center.x + rotated.x,
        transform.center.y + rotated.y,
    )
}

pub(super) fn world_to_local(transform: CanvasTransform, world: CanvasPoint) -> CanvasPoint {
    let relative = CanvasPoint::new(world.x - transform.center.x, world.y - transform.center.y);
    let rotated = rotate(relative, -transform.rotation);
    CanvasPoint::new(
        rotated.x / transform.size.x.max(0.001),
        rotated.y / transform.size.y.max(0.001),
    )
}

pub(super) fn entity_corners(entity: &CanvasEntity) -> [CanvasPoint; 4] {
    [
        local_to_world(entity.transform, CanvasPoint::new(-0.5, -0.5)),
        local_to_world(entity.transform, CanvasPoint::new(0.5, -0.5)),
        local_to_world(entity.transform, CanvasPoint::new(0.5, 0.5)),
        local_to_world(entity.transform, CanvasPoint::new(-0.5, 0.5)),
    ]
}

pub(super) fn entity_bounds(entity: &CanvasEntity) -> WorldRect {
    match &entity.kind {
        CanvasEntityKind::Line => {
            let start = local_to_world(entity.transform, CanvasPoint::new(-0.5, 0.0));
            let end = local_to_world(entity.transform, CanvasPoint::new(0.5, 0.0));
            WorldRect::from_points(start, end)
        }
        CanvasEntityKind::Pen { points } if !points.is_empty() => points
            .iter()
            .map(|point| local_to_world(entity.transform, *point))
            .map(|point| WorldRect::from_points(point, point))
            .reduce(WorldRect::union)
            .unwrap(),
        _ => {
            let corners = entity_corners(entity);
            corners
                .into_iter()
                .map(|point| WorldRect::from_points(point, point))
                .reduce(WorldRect::union)
                .unwrap()
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct DirectEditorLayout {
    pub(super) title_bar: WorldRect,
    pub(super) content: WorldRect,
}

pub(super) fn direct_editor_entity_size(intrinsic: Vec2, scale: f32) -> CanvasPoint {
    CanvasPoint::new(
        ((intrinsic.x + EMBEDDED_EDITOR_PADDING * 2.0) * scale).max(MIN_SIZE),
        ((intrinsic.y
            + EMBEDDED_EDITOR_PADDING * 2.0
            + EMBEDDED_EDITOR_TITLE_HEIGHT
            + EMBEDDED_EDITOR_TITLE_GAP)
            * scale)
            .max(MIN_SIZE),
    )
}

pub(super) fn direct_editor_to_preview(
    entity: &CanvasEntity,
    block_id: BlockRef,
) -> Option<CanvasEntity> {
    let content = direct_editor_layout(entity)?.content;
    let content_size = content.size();
    let mut preview = entity.clone();
    preview.kind = CanvasEntityKind::Block { block_id };
    preview.transform.center = content.center();
    preview.transform.size =
        CanvasPoint::new(content_size.x.max(MIN_SIZE), content_size.y.max(MIN_SIZE));
    preview.transform.rotation = 0.0;
    Some(preview)
}

pub(super) fn preview_to_direct_editor(
    entity: &CanvasEntity,
    block_id: BlockRef,
    intrinsic: Vec2,
) -> CanvasEntity {
    let content = entity_bounds(entity);
    let content_size = content.size();
    let scale = (content_size.x / intrinsic.x)
        .min(content_size.y / intrinsic.y)
        .max(f32::EPSILON);
    let mut direct = entity.clone();
    direct.kind = CanvasEntityKind::DirectEditor { block_id, scale };
    direct.transform.center = CanvasPoint::new(
        content.center().x,
        content.center().y
            - (EMBEDDED_EDITOR_TITLE_HEIGHT + EMBEDDED_EDITOR_TITLE_GAP) * scale * 0.5,
    );
    direct.transform.size =
        direct_editor_entity_size(Vec2::new(content_size.x, content_size.y), scale);
    direct.transform.rotation = 0.0;
    direct
}

pub(super) fn direct_editor_layout(entity: &CanvasEntity) -> Option<DirectEditorLayout> {
    let CanvasEntityKind::DirectEditor { scale, .. } = entity.kind else {
        return None;
    };
    let bounds = entity_bounds(entity);
    let padding = EMBEDDED_EDITOR_PADDING * scale;
    let title_height = EMBEDDED_EDITOR_TITLE_HEIGHT * scale;
    let title_gap = EMBEDDED_EDITOR_TITLE_GAP * scale;
    Some(DirectEditorLayout {
        title_bar: WorldRect {
            min: CanvasPoint::new(bounds.min.x + padding, bounds.min.y + padding),
            max: CanvasPoint::new(
                bounds.max.x - padding,
                bounds.min.y + padding + title_height,
            ),
        },
        content: WorldRect {
            min: CanvasPoint::new(
                bounds.min.x + padding,
                bounds.min.y + padding + title_height + title_gap,
            ),
            max: CanvasPoint::new(bounds.max.x - padding, bounds.max.y - padding),
        },
    })
}

pub(super) fn hit_entity(entity: &CanvasEntity, point: CanvasPoint, radius: f32) -> bool {
    match &entity.kind {
        CanvasEntityKind::Line => {
            let a = local_to_world(entity.transform, CanvasPoint::new(-0.5, 0.0));
            let b = local_to_world(entity.transform, CanvasPoint::new(0.5, 0.0));
            point_segment_distance(point, a, b) <= radius
        }
        CanvasEntityKind::Pen { points } => points.windows(2).any(|segment| {
            point_segment_distance(
                point,
                local_to_world(entity.transform, segment[0]),
                local_to_world(entity.transform, segment[1]),
            ) <= radius
        }),
        CanvasEntityKind::Rectangle if entity.style.fill.is_none() => {
            let local = world_to_local(entity.transform, point);
            let hit_radius = radius + entity.style.line_width * 0.5;
            let x_radius = hit_radius / entity.transform.size.x.max(MIN_SIZE);
            let y_radius = hit_radius / entity.transform.size.y.max(MIN_SIZE);
            let near_vertical_edge =
                (local.x.abs() - 0.5).abs() <= x_radius && local.y.abs() <= 0.5 + y_radius;
            let near_horizontal_edge =
                (local.y.abs() - 0.5).abs() <= y_radius && local.x.abs() <= 0.5 + x_radius;
            near_vertical_edge || near_horizontal_edge
        }
        _ => {
            let local = world_to_local(entity.transform, point);
            local.x.abs() <= 0.5 + radius / entity.transform.size.x.max(MIN_SIZE)
                && local.y.abs() <= 0.5 + radius / entity.transform.size.y.max(MIN_SIZE)
        }
    }
}

pub(super) fn point_segment_distance(point: CanvasPoint, a: CanvasPoint, b: CanvasPoint) -> f32 {
    let ab = CanvasPoint::new(b.x - a.x, b.y - a.y);
    let length_squared = ab.x * ab.x + ab.y * ab.y;
    if length_squared <= f32::EPSILON {
        return distance(point, a);
    }
    let projection =
        (((point.x - a.x) * ab.x + (point.y - a.y) * ab.y) / length_squared).clamp(0.0, 1.0);
    distance(
        point,
        CanvasPoint::new(a.x + ab.x * projection, a.y + ab.y * projection),
    )
}

pub(super) fn screen_rect(editor: &InfiniteCanvasEditor, bounds: WorldRect, rect: Rect) -> Rect {
    Rect::from_two_pos(
        editor.world_to_screen(bounds.min, rect),
        editor.world_to_screen(bounds.max, rect),
    )
}

pub(super) fn resize_handle_at(
    editor: &InfiniteCanvasEditor,
    frame: SelectionFrame,
    rect: Rect,
    world: CanvasPoint,
    resize: DirectEditorResize,
) -> Option<ResizeHandle> {
    let pointer = editor.world_to_screen(world, rect);
    resize_handles(frame)
        .into_iter()
        .filter(|(handle, _)| resize_handle_allowed(*handle, resize))
        .find_map(|(handle, point)| {
            (editor.world_to_screen(point, rect).distance(pointer) <= HANDLE_RADIUS + 3.0)
                .then_some(handle)
        })
}

pub(super) fn resize_handle_allowed(handle: ResizeHandle, resize: DirectEditorResize) -> bool {
    (handle.x == 0 || resize.horizontal()) && (handle.y == 0 || resize.vertical())
}

pub(super) fn resize_handles(frame: SelectionFrame) -> [(ResizeHandle, CanvasPoint); 8] {
    [
        (
            ResizeHandle { x: -1, y: -1 },
            frame.point(CanvasPoint::new(-0.5, -0.5)),
        ),
        (
            ResizeHandle { x: 0, y: -1 },
            frame.point(CanvasPoint::new(0.0, -0.5)),
        ),
        (
            ResizeHandle { x: 1, y: -1 },
            frame.point(CanvasPoint::new(0.5, -0.5)),
        ),
        (
            ResizeHandle { x: 1, y: 0 },
            frame.point(CanvasPoint::new(0.5, 0.0)),
        ),
        (
            ResizeHandle { x: 1, y: 1 },
            frame.point(CanvasPoint::new(0.5, 0.5)),
        ),
        (
            ResizeHandle { x: 0, y: 1 },
            frame.point(CanvasPoint::new(0.0, 0.5)),
        ),
        (
            ResizeHandle { x: -1, y: 1 },
            frame.point(CanvasPoint::new(-0.5, 0.5)),
        ),
        (
            ResizeHandle { x: -1, y: 0 },
            frame.point(CanvasPoint::new(-0.5, 0.0)),
        ),
    ]
}

pub(super) fn rotate_handle_at(
    editor: &InfiniteCanvasEditor,
    frame: SelectionFrame,
    rect: Rect,
) -> Pos2 {
    let top = editor.world_to_screen(frame.point(CanvasPoint::new(0.0, -0.5)), rect);
    top + Vec2::angled(frame.rotation - std::f32::consts::FRAC_PI_2) * ROTATE_OFFSET
}

pub(super) fn preview_entities(gesture: &Gesture) -> Vec<CanvasEntity> {
    match gesture {
        Gesture::Move {
            start,
            current,
            originals,
            ..
        } => {
            let delta = CanvasPoint::new(current.x - start.x, current.y - start.y);
            originals
                .iter()
                .cloned()
                .map(|mut entity| {
                    entity.transform.center.x += delta.x;
                    entity.transform.center.y += delta.y;
                    entity
                })
                .collect()
        }
        Gesture::Resize {
            handle,
            frame,
            current,
            originals,
            preserve_aspect_ratio,
            scale_text,
            ..
        } => resize_entities(
            *handle,
            *frame,
            *current,
            originals,
            *preserve_aspect_ratio,
            *scale_text,
        ),
        Gesture::Rotate {
            frame,
            start_angle,
            current,
            originals,
            snap_angle,
        } => {
            let center = frame.center;
            let current_angle = (current.y - center.y).atan2(current.x - center.x);
            let mut delta = current_angle - start_angle;
            if *snap_angle {
                let step = 15.0_f32.to_radians();
                delta = (delta / step).round() * step;
            }
            originals
                .iter()
                .cloned()
                .map(|mut entity| {
                    let relative = CanvasPoint::new(
                        entity.transform.center.x - center.x,
                        entity.transform.center.y - center.y,
                    );
                    let rotated = rotate(relative, delta);
                    entity.transform.center =
                        CanvasPoint::new(center.x + rotated.x, center.y + rotated.y);
                    entity.transform.rotation += delta;
                    entity
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

pub(super) fn constrain_point_angle(
    start: CanvasPoint,
    current: CanvasPoint,
    step: f32,
) -> CanvasPoint {
    let delta = CanvasPoint::new(current.x - start.x, current.y - start.y);
    let length = delta.x.hypot(delta.y);
    let angle = (delta.y.atan2(delta.x) / step).round() * step;
    CanvasPoint::new(
        start.x + length * angle.cos(),
        start.y + length * angle.sin(),
    )
}

pub(super) fn resize_entities(
    handle: ResizeHandle,
    frame: SelectionFrame,
    current: CanvasPoint,
    originals: &[CanvasEntity],
    preserve_aspect_ratio: bool,
    scale_text: bool,
) -> Vec<CanvasEntity> {
    let current = rotate(
        CanvasPoint::new(current.x - frame.center.x, current.y - frame.center.y),
        -frame.rotation,
    );
    let local_originals = originals
        .iter()
        .cloned()
        .map(|mut entity| {
            entity.transform.center = rotate(
                CanvasPoint::new(
                    entity.transform.center.x - frame.center.x,
                    entity.transform.center.y - frame.center.y,
                ),
                -frame.rotation,
            );
            entity.transform.rotation -= frame.rotation;
            entity
        })
        .collect::<Vec<_>>();
    resize_entities_axis(
        handle,
        frame.local_bounds(),
        current,
        &local_originals,
        preserve_aspect_ratio,
        scale_text,
    )
    .into_iter()
    .map(|mut entity| {
        let center = rotate(entity.transform.center, frame.rotation);
        entity.transform.center =
            CanvasPoint::new(frame.center.x + center.x, frame.center.y + center.y);
        entity.transform.rotation += frame.rotation;
        entity
    })
    .collect()
}

pub(super) fn resize_entities_axis(
    handle: ResizeHandle,
    bounds: WorldRect,
    current: CanvasPoint,
    originals: &[CanvasEntity],
    preserve_aspect_ratio: bool,
    scale_text: bool,
) -> Vec<CanvasEntity> {
    let original_size = bounds.size();
    let mut resized = bounds;
    if handle.x < 0 {
        resized.min.x = current.x.min(resized.max.x - MIN_SIZE);
    } else if handle.x > 0 {
        resized.max.x = current.x.max(resized.min.x + MIN_SIZE);
    }
    if handle.y < 0 {
        resized.min.y = current.y.min(resized.max.y - MIN_SIZE);
    } else if handle.y > 0 {
        resized.max.y = current.y.max(resized.min.y + MIN_SIZE);
    }
    if preserve_aspect_ratio {
        resized = proportional_resize_bounds(handle, bounds, resized);
    }
    let resized_size = resized.size();
    let scale_x = if handle.x == 0 && !preserve_aspect_ratio {
        1.0
    } else {
        resized_size.x / original_size.x.max(MIN_SIZE)
    };
    let scale_y = if handle.y == 0 && !preserve_aspect_ratio {
        1.0
    } else {
        resized_size.y / original_size.y.max(MIN_SIZE)
    };

    originals
        .iter()
        .cloned()
        .map(|mut entity| {
            if matches!(entity.kind, CanvasEntityKind::Line) {
                let start = local_to_world(entity.transform, CanvasPoint::new(-0.5, 0.0));
                let end = local_to_world(entity.transform, CanvasPoint::new(0.5, 0.0));
                let resize_point = |point: CanvasPoint| {
                    CanvasPoint::new(
                        if handle.x == 0 && !preserve_aspect_ratio {
                            point.x
                        } else {
                            resized.min.x
                                + (point.x - bounds.min.x) / original_size.x.max(MIN_SIZE)
                                    * resized_size.x
                        },
                        if handle.y == 0 && !preserve_aspect_ratio {
                            point.y
                        } else {
                            resized.min.y
                                + (point.y - bounds.min.y) / original_size.y.max(MIN_SIZE)
                                    * resized_size.y
                        },
                    )
                };
                let start = resize_point(start);
                let end = resize_point(end);
                let delta = CanvasPoint::new(end.x - start.x, end.y - start.y);
                entity.transform.center = midpoint(start, end);
                entity.transform.size.x = distance(start, end).max(MIN_SIZE);
                entity.transform.rotation = delta.y.atan2(delta.x);
                return entity;
            }
            let unit_x = (entity.transform.center.x - bounds.min.x) / original_size.x.max(MIN_SIZE);
            let unit_y = (entity.transform.center.y - bounds.min.y) / original_size.y.max(MIN_SIZE);
            entity.transform.center = CanvasPoint::new(
                if handle.x == 0 && !preserve_aspect_ratio {
                    entity.transform.center.x
                } else {
                    resized.min.x + unit_x * resized_size.x
                },
                if handle.y == 0 && !preserve_aspect_ratio {
                    entity.transform.center.y
                } else {
                    resized.min.y + unit_y * resized_size.y
                },
            );
            entity.transform.size.x = (entity.transform.size.x * scale_x).max(MIN_SIZE);
            entity.transform.size.y = (entity.transform.size.y * scale_y).max(MIN_SIZE);
            if let CanvasEntityKind::Text { text_style, .. } = &mut entity.kind {
                if scale_text {
                    let factor = if handle.x == 0 { scale_y } else { scale_x };
                    text_style.font_size = (text_style.font_size * factor).max(4.0);
                } else {
                    text_style.wrap = true;
                }
            }
            entity
        })
        .collect()
}

pub(super) fn proportional_resize_bounds(
    handle: ResizeHandle,
    bounds: WorldRect,
    resized: WorldRect,
) -> WorldRect {
    let original_width = bounds.size().x.max(MIN_SIZE);
    let original_height = bounds.size().y.max(MIN_SIZE);
    let resized_width = resized.size().x.max(MIN_SIZE);
    let resized_height = resized.size().y.max(MIN_SIZE);
    let scale = match (handle.x, handle.y) {
        (0, _) => resized_height / original_height,
        (_, 0) => resized_width / original_width,
        _ => {
            let horizontal = resized_width / original_width;
            let vertical = resized_height / original_height;
            if (horizontal - 1.0).abs() >= (vertical - 1.0).abs() {
                horizontal
            } else {
                vertical
            }
        }
    }
    .max(MIN_SIZE / original_width)
    .max(MIN_SIZE / original_height);
    let width = original_width * scale;
    let height = original_height * scale;
    let center = bounds.center();
    let (min_x, max_x) = match handle.x {
        -1 => (bounds.max.x - width, bounds.max.x),
        1 => (bounds.min.x, bounds.min.x + width),
        _ => (center.x - width * 0.5, center.x + width * 0.5),
    };
    let (min_y, max_y) = match handle.y {
        -1 => (bounds.max.y - height, bounds.max.y),
        1 => (bounds.min.y, bounds.min.y + height),
        _ => (center.y - height * 0.5, center.y + height * 0.5),
    };
    WorldRect {
        min: CanvasPoint::new(min_x, min_y),
        max: CanvasPoint::new(max_x, max_y),
    }
}

pub(super) fn pen_entity(points: Vec<CanvasPoint>, style: CanvasEntityStyle) -> CanvasEntity {
    let bounds = points
        .iter()
        .copied()
        .map(|point| WorldRect::from_points(point, point))
        .reduce(WorldRect::union)
        .unwrap();
    let center = bounds.center();
    let size = CanvasPoint::new(bounds.size().x.max(MIN_SIZE), bounds.size().y.max(MIN_SIZE));
    let points = points
        .into_iter()
        .map(|point| CanvasPoint::new((point.x - center.x) / size.x, (point.y - center.y) / size.y))
        .collect();
    CanvasEntity {
        id: Uuid::new_v4(),
        transform: CanvasTransform::new(center, size, 0.0),
        kind: CanvasEntityKind::Pen { points },
        style,
        group_id: None,
        locked: false,
        components: Vec::new(),
    }
}
