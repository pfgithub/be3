use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_entity(
    editor: &InfiniteCanvasEditor,
    painter: &egui::Painter,
    rect: Rect,
    entity: &CanvasEntity,
    dependency_details: &HashMap<Uuid, (String, Uuid)>,
    editors: &mut EditorAccess<'_>,
    live_editor_overlay: bool,
    parent_opacity: f32,
) {
    let auto = painter.ctx().global_style().visuals.text_color();
    let opacity = (entity.style.opacity * parent_opacity).clamp(0.0, 1.0);
    let color = with_opacity(resolve_color(entity.style.foreground, auto), opacity);
    let stroke = Stroke::new(
        (entity.style.line_width.max(0.0) * editor.render_scale).max(0.1),
        color,
    );
    match &entity.kind {
        CanvasEntityKind::Line => {
            paint_styled_line(
                painter,
                editor.world_to_screen(
                    local_to_world(entity.transform, CanvasPoint::new(-0.5, 0.0)),
                    rect,
                ),
                editor.world_to_screen(
                    local_to_world(entity.transform, CanvasPoint::new(0.5, 0.0)),
                    rect,
                ),
                stroke,
                entity.style,
                editor.render_scale,
            );
        }
        CanvasEntityKind::Rectangle => {
            let points: Vec<_> = rounded_rectangle_points(entity, entity.style.corner_radius)
                .into_iter()
                .map(|point| editor.world_to_screen(point, rect))
                .collect();
            let fill = entity
                .style
                .fill
                .map(|fill| with_opacity(resolve_color(fill, auto), opacity))
                .unwrap_or(Color32::TRANSPARENT);
            painter.add(egui::Shape::convex_polygon(points, fill, stroke));
        }
        CanvasEntityKind::Text { text, text_style } => {
            let center = editor.world_to_screen(entity.transform.center, rect);
            let font_size = (text_style.font_size * editor.render_scale).clamp(4.0, 256.0);
            let font = egui::FontId::proportional(font_size);
            let wrap_width = if text_style.wrap {
                (entity.transform.size.x * editor.render_scale).max(1.0)
            } else {
                f32::INFINITY
            };
            let mut job = egui::text::LayoutJob::simple(text.clone(), font, color, wrap_width);
            job.halign = match text_style.alignment {
                CanvasTextAlign::Left => egui::Align::LEFT,
                CanvasTextAlign::Center => egui::Align::Center,
                CanvasTextAlign::Right => egui::Align::RIGHT,
            };
            job.sections[0].format.line_height = Some(font_size * text_style.line_height.max(0.5));
            let galley = painter.layout_job(job);
            let position = center - galley.size() * 0.5;
            if text_style.weight == CanvasTextWeight::Bold {
                painter.add(
                    egui::epaint::TextShape::new(
                        position + Vec2::new(0.6 * editor.render_scale, 0.0),
                        galley.clone(),
                        color,
                    )
                    .with_angle_and_anchor(entity.transform.rotation, egui::Align2::CENTER_CENTER),
                );
            }
            painter.add(
                egui::epaint::TextShape::new(position, galley, color)
                    .with_angle_and_anchor(entity.transform.rotation, egui::Align2::CENTER_CENTER),
            );
        }
        CanvasEntityKind::Pen { points } => {
            painter.add(egui::Shape::line(
                points
                    .iter()
                    .map(|point| {
                        editor.world_to_screen(local_to_world(entity.transform, *point), rect)
                    })
                    .collect(),
                stroke,
            ));
        }
        CanvasEntityKind::Block { block_id } => {
            let corners = entity_corners(entity).map(|point| editor.world_to_screen(point, rect));
            if editors.render(
                *block_id,
                BlockRenderContext {
                    painter,
                    corners,
                    opacity,
                },
            ) {
                return;
            }
            painter.add(egui::Shape::convex_polygon(
                corners.to_vec(),
                with_opacity(Color32::from_gray(35), opacity),
                Stroke::NONE,
            ));
            let center = editor.world_to_screen(entity.transform.center, rect);
            let title = dependency_details
                .get(block_id)
                .map(|(title, _)| title.clone())
                .unwrap_or_else(|| "Loading…".into());
            let title_galley = painter.layout_no_wrap(
                title,
                egui::FontId::proportional((18.0 * editor.render_scale).clamp(8.0, 42.0)),
                color,
            );
            let preview_galley = painter.layout_no_wrap(
                "(TODO: preview)".into(),
                egui::FontId::proportional((12.0 * editor.render_scale).clamp(7.0, 30.0)),
                color,
            );
            let gap = (4.0 * editor.render_scale).clamp(2.0, 10.0);
            let total_height = title_galley.size().y + gap + preview_galley.size().y;
            let title_center_offset = -total_height * 0.5 + title_galley.size().y * 0.5;
            let preview_center_offset = total_height * 0.5 - preview_galley.size().y * 0.5;
            let (sin, cos) = entity.transform.rotation.sin_cos();
            let rotated_offset = |offset: f32| Vec2::new(-offset * sin, offset * cos);
            let title_center = center + rotated_offset(title_center_offset);
            let preview_center = center + rotated_offset(preview_center_offset);
            painter.add(
                egui::epaint::TextShape::new(
                    title_center - title_galley.size() * 0.5,
                    title_galley,
                    color,
                )
                .with_angle_and_anchor(entity.transform.rotation, egui::Align2::CENTER_CENTER),
            );
            painter.add(
                egui::epaint::TextShape::new(
                    preview_center - preview_galley.size() * 0.5,
                    preview_galley,
                    color,
                )
                .with_angle_and_anchor(entity.transform.rotation, egui::Align2::CENTER_CENTER),
            );
        }
        CanvasEntityKind::DirectEditor { block_id, scale } => {
            let Some(layout) = direct_editor_layout(entity) else {
                return;
            };
            let outer = screen_rect(editor, entity_bounds(entity), rect);
            let title_bar = screen_rect(editor, layout.title_bar, rect);
            let content = screen_rect(editor, layout.content, rect);
            let visuals = &painter.ctx().global_style().visuals;
            painter.rect(
                outer,
                (6.0 * scale * editor.render_scale).clamp(2.0, 12.0),
                with_opacity(visuals.panel_fill, opacity),
                Stroke::new(
                    editor.render_scale.max(0.5),
                    with_opacity(visuals.widgets.noninteractive.bg_stroke.color, opacity),
                ),
                egui::StrokeKind::Inside,
            );
            painter.rect_filled(
                title_bar,
                (4.0 * scale * editor.render_scale).clamp(1.0, 8.0),
                with_opacity(visuals.widgets.inactive.bg_fill, opacity),
            );

            let content_corners = [
                content.left_top(),
                content.right_top(),
                content.right_bottom(),
                content.left_bottom(),
            ];
            let preview = !live_editor_overlay
                || (editors.direct_editor_interaction(*block_id)
                    == Some(DirectEditorInteraction::Preview)
                    && editor.focused_editor != Some(entity.id));
            if preview
                && !editors.render(
                    *block_id,
                    BlockRenderContext {
                        painter,
                        corners: content_corners,
                        opacity,
                    },
                )
            {
                painter.rect_filled(content, 0.0, with_opacity(Color32::from_gray(35), opacity));
            }

            let (title, block_type) = dependency_details
                .get(block_id)
                .map(|(title, block_type)| (title.as_str(), Some(*block_type)))
                .unwrap_or(("Loading...", None));
            let font_size = (16.0 * scale * editor.render_scale).clamp(8.0, 32.0);
            let left_padding = (6.0 * scale * editor.render_scale).clamp(3.0, 12.0);
            let title_painter = painter.with_clip_rect(title_bar);
            let mut title_x = title_bar.left() + left_padding;
            if let Some(icon) =
                block_type.and_then(|block_type| editors.registry().icon(block_type))
            {
                title_painter.text(
                    Pos2::new(title_x, title_bar.center().y),
                    egui::Align2::LEFT_CENTER,
                    icon.codepoint,
                    egui::FontId::new(font_size, icon.font_family()),
                    color,
                );
                title_x += font_size + left_padding;
            }
            title_painter.text(
                Pos2::new(title_x, title_bar.center().y),
                egui::Align2::LEFT_CENTER,
                title,
                egui::FontId::proportional(font_size),
                color,
            );
        }
    }
}

pub(super) fn paint_styled_line(
    painter: &egui::Painter,
    start: Pos2,
    end: Pos2,
    stroke: Stroke,
    style: CanvasEntityStyle,
    zoom: f32,
) {
    let delta = end - start;
    let length = delta.length();
    if length <= f32::EPSILON {
        return;
    }
    let direction = delta / length;
    let arrow_size = ((style.line_width * 4.0).max(8.0) * zoom).min(length * 0.4);
    let inset = arrow_size * 0.75;
    let shaft_start = if style.arrow_start {
        start + direction * inset
    } else {
        start
    };
    let shaft_end = if style.arrow_end {
        end - direction * inset
    } else {
        end
    };

    if style.dashed {
        let dash = (stroke.width * 3.0).max(2.0);
        let gap = (stroke.width * 2.0).max(2.0);
        painter.extend(egui::Shape::dashed_line(
            &[shaft_start, shaft_end],
            stroke,
            dash,
            gap,
        ));
    } else {
        painter.line_segment([shaft_start, shaft_end], stroke);
    }

    if style.arrow_start {
        painter.add(arrowhead(start, direction, arrow_size, stroke.color));
    }
    if style.arrow_end {
        painter.add(arrowhead(end, -direction, arrow_size, stroke.color));
    }
}

pub(super) fn arrowhead(tip: Pos2, inward: Vec2, size: f32, color: Color32) -> egui::Shape {
    let base = tip + inward * size;
    let perpendicular = Vec2::new(-inward.y, inward.x) * size * 0.45;
    egui::Shape::convex_polygon(
        vec![tip, base + perpendicular, base - perpendicular],
        color,
        Stroke::NONE,
    )
}

pub(super) fn rounded_rectangle_points(entity: &CanvasEntity, radius: f32) -> Vec<CanvasPoint> {
    let width = entity.transform.size.x.abs().max(0.001);
    let height = entity.transform.size.y.abs().max(0.001);
    let radius = radius.max(0.0).min(width * 0.5).min(height * 0.5);
    if radius <= f32::EPSILON {
        return entity_corners(entity).into();
    }

    const STEPS: usize = 6;
    let corners = [
        (width * 0.5 - radius, -height * 0.5 + radius, -90.0_f32),
        (width * 0.5 - radius, height * 0.5 - radius, 0.0),
        (-width * 0.5 + radius, height * 0.5 - radius, 90.0),
        (-width * 0.5 + radius, -height * 0.5 + radius, 180.0),
    ];
    let mut points = Vec::with_capacity(corners.len() * (STEPS + 1));
    for (center_x, center_y, start_degrees) in corners {
        for step in 0..=STEPS {
            let angle = (start_degrees + 90.0 * step as f32 / STEPS as f32).to_radians();
            let local_x = center_x + radius * angle.cos();
            let local_y = center_y + radius * angle.sin();
            points.push(local_to_world(
                entity.transform,
                CanvasPoint::new(local_x / width, local_y / height),
            ));
        }
    }
    points
}

pub(super) fn with_opacity(color: Color32, opacity: f32) -> Color32 {
    let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(
        red,
        green,
        blue,
        (alpha as f32 * opacity.clamp(0.0, 1.0)).round() as u8,
    )
}

pub(super) fn paint_selection(
    editor: &InfiniteCanvasEditor,
    painter: &egui::Painter,
    rect: Rect,
    frame: SelectionFrame,
    resize: DirectEditorResize,
    allow_rotation: bool,
) {
    let corners = [
        CanvasPoint::new(-0.5, -0.5),
        CanvasPoint::new(0.5, -0.5),
        CanvasPoint::new(0.5, 0.5),
        CanvasPoint::new(-0.5, 0.5),
    ]
    .map(|point| editor.world_to_screen(frame.point(point), rect));
    painter.add(egui::Shape::closed_line(
        corners.to_vec(),
        Stroke::new(1.0_f32, Color32::LIGHT_BLUE),
    ));
    if resize != DirectEditorResize::None {
        for (handle, point) in resize_handles(frame) {
            if !resize_handle_allowed(handle, resize) {
                continue;
            }
            painter.circle_filled(
                editor.world_to_screen(point, rect),
                HANDLE_RADIUS,
                Color32::LIGHT_BLUE,
            );
        }
    }
    if allow_rotation {
        let rotate = rotate_handle_at(editor, frame, rect);
        let top = editor.world_to_screen(frame.point(CanvasPoint::new(0.0, -0.5)), rect);
        painter.line_segment([top, rotate], Stroke::new(1.0_f32, Color32::LIGHT_BLUE));
        painter.circle_filled(rotate, HANDLE_RADIUS, Color32::LIGHT_BLUE);
    }
}
