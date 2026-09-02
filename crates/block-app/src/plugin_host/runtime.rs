use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use block_plugin_api::{
    ArtifactDescription, BlockPick, EditorInstanceId, EditorMessage, EditorRegion, Message,
    PluginManifest, ScreenId, ScreenLayout, ScreenRequest, ViewChange,
};
use eframe::egui;
use uuid::Uuid;

use super::{
    backend::{Availability, Backend, Platform},
    input,
    instances::{Instances, Placement},
    presenter::{
        self, PresenterCallback, PresenterState, PresenterStatus, Quad, Shared, MAX_SURFACES,
    },
    preview_size, ArtifactSlot, ArtifactState, BlockPickRequest, CreationSlot, CreationState,
    EditorBlock, EditorSlot, HostChild, HostChildStatus, InstanceRole, PreviewPresentation,
    PreviewSlot, RuntimeStatus, SurfaceStatus,
};

const CROWDED: &str = "Too many plugin runtimes are already presenting.";
const UNIT: egui::Rect = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
const FRAME_TIMEOUT_SECONDS: f64 = 1.0;

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::new());
}

struct Host {
    availability: Availability,
    runtimes: HashMap<String, Runtime>,
    grabbed: bool,
}

impl Host {
    fn new() -> Self {
        Self {
            availability: Availability::missing(),
            runtimes: HashMap::new(),
            grabbed: false,
        }
    }

    fn runtime(
        &mut self,
        plugin: &PluginManifest,
        context: &egui::Context,
    ) -> Result<&mut Runtime, String> {
        if let Err(error) = &self.availability.0 {
            return Err(error.clone());
        }
        let Some(surface) = self.surface_for(&plugin.identity.id, context) else {
            return Err(CROWDED.to_owned());
        };
        let runtime = self
            .runtimes
            .entry(plugin.identity.id.clone())
            .or_insert_with(|| Runtime::new(plugin, surface, context));
        runtime.begin_pass(context.cumulative_pass_nr());
        Ok(runtime)
    }

    fn surface_for(&mut self, plugin_id: &str, context: &egui::Context) -> Option<u32> {
        if let Some(runtime) = self.runtimes.get(plugin_id) {
            return Some(runtime.surface);
        }
        if let Some(surface) = (0..MAX_SURFACES).find(|surface| {
            !self
                .runtimes
                .values()
                .any(|runtime| runtime.surface == *surface)
        }) {
            return Some(surface);
        }
        let evicted = self
            .runtimes
            .iter()
            .filter(|(_, runtime)| runtime.instances.is_empty())
            .min_by_key(|(_, runtime)| runtime.pass)
            .map(|(id, _)| id.clone())?;
        self.shutdown(context, &evicted)
    }

    fn shutdown(&mut self, context: &egui::Context, plugin_id: &str) -> Option<u32> {
        let mut runtime = self.runtimes.remove(plugin_id)?;
        runtime.backend.shutdown();
        context
            .debug_painter()
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO),
                PresenterCallback::release(runtime.surface, runtime.status.clone()),
            ));
        Some(runtime.surface)
    }
}

pub(super) struct Runtime {
    pub(super) context: egui::Context,
    pub(super) backend: Platform,
    pub(super) instances: Instances,
    pub(super) layout: ScreenLayout,
    pub(super) pass: u64,
    surface: u32,
    status: PresenterStatus,
    shared: Arc<Mutex<Shared>>,
    presented: bool,
    sent: Vec<ScreenRequest>,
    error: Option<String>,
    needed: bool,
    next_slot: u32,
    paint_at: Option<f64>,
    requested_at: Option<f64>,
}

impl Runtime {
    fn new(plugin: &PluginManifest, surface: u32, context: &egui::Context) -> Self {
        let mut backend = Platform::new(plugin, context);
        backend.start(plugin, context);
        let mut instances = Instances::default();
        instances.allow_network(plugin.network.clone());
        Self {
            context: context.clone(),
            backend,
            instances,
            layout: ScreenLayout::default(),
            pass: 0,
            surface,
            status: PresenterStatus::waiting(),
            shared: Arc::new(Mutex::new(Shared::default())),
            presented: false,
            sent: Vec::new(),
            error: None,
            needed: false,
            next_slot: 0,
            paint_at: None,
            requested_at: None,
        }
    }

    fn restart(&mut self, plugin: &PluginManifest) {
        self.error = None;
        self.status = PresenterStatus::waiting();
        self.layout = ScreenLayout::default();
        self.sent.clear();
        self.needed = false;
        self.paint_at = None;
        self.requested_at = None;
        self.instances.reopen();
        self.backend.start(plugin, &self.context);
    }

    fn begin_pass(&mut self, pass: u64) {
        self.detect_error();
        if self.pass == pass || self.error.is_some() {
            return;
        }
        let previous = self.pass;
        self.pass = pass;
        self.next_slot = 0;
        let next = self.instances.next_screens(previous);
        let mut messages = next.opened;
        if self.sent != next.screens {
            self.sent.clone_from(&next.screens);
            messages.push(self.instances.screen_set(next.screens));
        }
        messages.extend(self.instances.pending());
        let awaited = messages.iter().any(|message| {
            matches!(
                message,
                Message::Editor(EditorMessage::Open { .. } | EditorMessage::OpenArtifact { .. })
            )
        });
        self.needed |= !messages.is_empty();
        self.send(messages);
        if awaited {
            self.context.request_repaint();
        }
        self.pump();
    }

    fn begin_frame(&mut self, frame: &eframe::Frame, pass: u64) {
        if self.error.is_some() || self.pass + 1 < pass {
            return;
        }
        let mut messages = match self.pass + 1 == pass {
            true => self.instances.frame_input(&self.context, self.pass),
            false => Vec::new(),
        };
        messages.extend(
            self.instances
                .drive_web_views(frame, &self.context, self.pass),
        );
        self.needed |= !messages.is_empty();
        if self.frame_due() {
            self.requested_at = Some(self.now());
            messages.push(Message::DrawFrame);
        }
        self.send(messages);
    }

    fn frame_due(&self) -> bool {
        let now = self.now();
        (self.needed || self.paint_at.is_some_and(|at| at <= now))
            && self
                .requested_at
                .is_none_or(|at| now - at >= FRAME_TIMEOUT_SECONDS)
    }

    fn now(&self) -> f64 {
        self.context.input(|input| input.time)
    }

    fn detect_error(&mut self) {
        if self.error.is_some() {
            return;
        }
        self.error = self.backend.take_error().or(match self.status.get() {
            PresenterState::Failed(error) | PresenterState::Unsupported(error) => Some(error),
            PresenterState::Waiting | PresenterState::Presenting | PresenterState::Released => None,
        });
    }

    fn pump(&mut self) {
        if self.error.is_some() {
            return;
        }
        let received = self.backend.receive();
        self.apply(received);
        let responses = self.instances.client_responses();
        self.send(responses);
    }

    pub(super) fn apply(&mut self, messages: Vec<Message>) {
        if messages.is_empty() {
            return;
        }
        let mut answers = Vec::new();
        let mut changed = false;
        for message in messages {
            changed |= match message {
                Message::Layout(layout) => {
                    self.layout = layout;
                    true
                }
                Message::Client(message) => {
                    self.instances.client_message(message);
                    false
                }
                Message::Editor(message) => self.instances.editor_message(message),
                Message::FrameNeeded => {
                    self.needed = true;
                    true
                }
                Message::FrameReady(frame) => {
                    self.await_next_frame(frame.repaint_after_micros);
                    false
                }
                Message::RegionSizes(sizes) => self.instances.set_region_sizes(sizes),
                Message::Frames(reports) => self.instances.set_frame_reports(reports),
                Message::Children(placements) => {
                    let (answered, changed) = self.instances.set_children(placements);
                    answers.extend(answered);
                    changed
                }
                _ => false,
            };
        }
        self.send(answers);
        if changed {
            self.context.request_repaint();
        }
    }

    fn await_next_frame(&mut self, repaint_after_micros: Option<u64>) {
        self.needed = false;
        self.requested_at = None;
        self.paint_at = repaint_after_micros.map(|micros| {
            let delay = Duration::from_micros(micros);
            self.context.request_repaint_after(delay);
            self.now() + delay.as_secs_f64()
        });
    }

    fn send(&mut self, messages: Vec<Message>) {
        if messages.is_empty() {
            return;
        }
        self.backend.send(messages);
    }

    fn present(
        &mut self,
        rect: egui::Rect,
        screen: ScreenId,
        quad: Quad,
        source: egui::Rect,
    ) -> egui::epaint::PaintCallback {
        self.presented = true;
        let slot = self.next_slot;
        self.next_slot = (self.next_slot + 1).min(presenter::MAX_REGIONS - 1);
        eframe::egui_wgpu::Callback::new_paint_callback(
            rect,
            PresenterCallback::present(
                self.surface,
                self.status.clone(),
                Arc::clone(&self.shared),
                screen,
                quad,
                source,
                slot,
            ),
        )
    }

    fn flush(&mut self) {
        self.pump();
        if !std::mem::take(&mut self.presented) {
            return;
        }
        let frame = self.backend.frame(&self.layout, self.pass);
        self.shared.lock().unwrap().publish(&self.layout, frame);
    }

    fn state(&self) -> String {
        match (&self.error, self.status.get()) {
            (Some(error), _) => error.clone(),
            (None, PresenterState::Presenting) => "presenting".to_owned(),
            (None, PresenterState::Released) => "released".to_owned(),
            (None, PresenterState::Failed(error) | PresenterState::Unsupported(error)) => error,
            (None, PresenterState::Waiting) => self.backend.state().to_owned(),
        }
    }
}

pub(crate) fn install(creation_context: &eframe::CreationContext<'_>) {
    let availability = presenter::install(creation_context);
    HOST.with(|host| {
        host.borrow_mut().availability = availability;
    });
}

fn plugin_loading_rect(
    layout: &ScreenLayout,
    screen: ScreenId,
    rect: egui::Rect,
) -> Option<egui::Rect> {
    layout.placement(screen).is_none().then_some(rect)
}

pub(crate) struct HostFrame {
    pub(crate) content: egui::Rect,
}

pub(crate) struct EditorPresentation {
    plugin_id: String,
    instance: EditorInstanceId,
    region: EditorRegion,
    pub(crate) id: Option<egui::Id>,
    pub(crate) loading_rect: Option<egui::Rect>,
    screen: Option<ScreenId>,
    quad: Option<Quad>,
    clip: egui::Rect,
    floating: Vec<egui::Rect>,
    pub(crate) open: Option<(Uuid, Uuid)>,
    pub(crate) children: Vec<HostChild>,
}

impl EditorPresentation {
    fn empty(plugin_id: &str, instance: EditorInstanceId, region: EditorRegion) -> Self {
        Self {
            plugin_id: plugin_id.to_owned(),
            instance,
            region,
            id: None,
            loading_rect: None,
            screen: None,
            quad: None,
            clip: egui::Rect::ZERO,
            floating: Vec::new(),
            open: None,
            children: Vec::new(),
        }
    }

    fn blit(&self, ui: &mut egui::Ui, rects: &[egui::Rect]) {
        let (Some(screen), Some(quad)) = (self.screen, self.quad) else {
            return;
        };
        let base = quad.rect;
        if !base.is_positive() {
            return;
        }
        with(&self.plugin_id, |runtime| {
            for rect in rects {
                let piece = rect.intersect(base).intersect(self.clip);
                if !piece.is_positive() {
                    continue;
                }
                let source = egui::Rect::from_min_max(
                    egui::pos2(
                        (piece.min.x - base.min.x) / base.width(),
                        (piece.min.y - base.min.y) / base.height(),
                    ),
                    egui::pos2(
                        (piece.max.x - base.min.x) / base.width(),
                        (piece.max.y - base.min.y) / base.height(),
                    ),
                );
                let callback = runtime.present(piece, screen, Quad::upright(piece), source);
                ui.painter().with_clip_rect(self.clip).add(callback);
            }
        });
    }

    pub(crate) fn present(&self, ui: &mut egui::Ui) {
        let Some(quad) = self.quad else {
            return;
        };
        let base = match self.floating.is_empty() {
            true => vec![quad.rect],
            false => super::pieces::subtract(quad.rect, &self.floating),
        };
        self.blit(ui, &base);
    }

    pub(crate) fn present_floating(&self, ui: &mut egui::Ui) {
        if self.floating.is_empty() {
            return;
        }
        let floating = self.floating.clone();
        self.blit(ui, &floating);
    }

    pub(crate) fn report(&self, statuses: Vec<HostChildStatus>) {
        report_children(&self.plugin_id, self.instance, self.region, statuses);
    }
}

pub(crate) fn editor_ui(ui: &mut egui::Ui, slot: EditorSlot<'_>) -> EditorPresentation {
    let EditorSlot {
        plugin,
        block_types,
        client,
        client_id,
        role,
        instance,
        region,
        frame,
        size,
        view,
    } = slot;
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let runtime = match host.runtime(plugin, ui.ctx()) {
            Ok(runtime) => runtime,
            Err(error) => {
                ui.colored_label(egui::Color32::RED, error);
                return EditorPresentation::empty(&plugin.identity.id, instance, region);
            }
        };
        if let Some(error) = runtime.error.clone() {
            ui.colored_label(egui::Color32::RED, error);
            if ui.button("Restart plugin").clicked() {
                runtime.restart(plugin);
            }
            return EditorPresentation::empty(&plugin.identity.id, instance, region);
        }
        let pass = runtime.pass;
        let response = ui.allocate_response(size, egui::Sense::click_and_drag());
        let cropped = Quad::upright(response.rect).crop_to(ui.clip_rect());
        let visible = cropped.as_ref().map_or(egui::Rect::ZERO, |(_, source)| {
            scale_rect(*source, response.rect.size())
        });
        let screen = runtime.instances.report(
            instance,
            region,
            ui.ctx(),
            &client,
            client_id,
            role,
            block_types,
            frame,
            response.rect.size(),
            visible,
            ui.ctx().pixels_per_point(),
            pass,
        );
        if let Some(view) = view {
            runtime.instances.set_view(instance, view);
        }
        let (children, holes) =
            runtime
                .instances
                .host_children(instance, region, response.rect, ui.clip_rect());
        runtime.instances.place(
            instance,
            region,
            Placement {
                id: response.id,
                rect: response.rect,
                clip: ui.clip_rect(),
                pass,
            },
        );
        let over_hole = ui
            .ctx()
            .pointer_latest_pos()
            .is_some_and(|position| holes.contains(position));
        let drag = input::block_drag(&response).filter(|_| !over_hole);
        let hovering = drag.as_ref().is_some_and(|drag| !drag.dropped);
        let messages = runtime.instances.drag(instance, region, drag);
        runtime.send(messages);
        let files = input::file_drop(&response).filter(|_| !over_hole);
        let messages = runtime.instances.file_drop(instance, region, files);
        runtime.send(messages);
        if hovering && runtime.instances.drag_accepted(instance) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Alias);
        } else if response.hovered() && !over_hole {
            if let Some(cursor) = runtime.instances.cursor(instance, region) {
                ui.ctx().set_cursor_icon(cursor);
            }
        }
        EditorPresentation {
            plugin_id: plugin.identity.id.clone(),
            instance,
            region,
            id: Some(response.id),
            loading_rect: plugin_loading_rect(&runtime.layout, screen, response.rect),
            screen: Some(screen),
            quad: cropped.map(|(quad, _)| quad),
            clip: ui.clip_rect(),
            floating: runtime
                .instances
                .frame_report(instance)
                .map(|report| {
                    report
                        .floating
                        .iter()
                        .map(|rect| {
                            egui::Rect::from_min_size(
                                egui::pos2(rect.x, rect.y) + response.rect.min.to_vec2(),
                                egui::vec2(rect.width, rect.height),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            open: runtime.instances.take_open(instance),
            children,
        }
    })
}

pub(crate) fn report_children(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    statuses: Vec<HostChildStatus>,
) {
    with(plugin_id, |runtime| {
        let messages = runtime
            .instances
            .set_child_statuses(instance, region, statuses);
        runtime.send(messages);
    });
}

pub(crate) fn take_block_pick(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Option<BlockPickRequest> {
    with(plugin_id, |runtime| {
        runtime.instances.take_block_pick(instance)
    })
    .flatten()
}

pub(crate) fn block_picked(
    plugin_id: &str,
    instance: EditorInstanceId,
    request_id: u64,
    pick: BlockPick,
) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.block_picked(instance, request_id, pick);
        runtime.send(messages);
    });
}

fn scale_rect(rect: egui::Rect, size: egui::Vec2) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(rect.min.x * size.x, rect.min.y * size.y),
        egui::pos2(rect.max.x * size.x, rect.max.y * size.y),
    )
}

pub(crate) fn creation(context: &egui::Context, slot: CreationSlot<'_>) -> CreationState {
    let CreationSlot {
        plugin,
        block_types,
        client,
        client_id,
        instance,
    } = slot;
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let runtime = match host.runtime(plugin, context) {
            Ok(runtime) => runtime,
            Err(error) => return CreationState::Failed(error),
        };
        if let Some(error) = runtime.error.clone() {
            return CreationState::Failed(error);
        }
        match runtime
            .instances
            .report_creation(instance, context, &client, client_id, block_types)
        {
            true => CreationState::Ready,
            false => CreationState::Starting,
        }
    })
}

pub(crate) fn artifact(context: &egui::Context, slot: ArtifactSlot<'_>) -> ArtifactState {
    let ArtifactSlot {
        plugin,
        block_types,
        client,
        client_id,
        instance,
        block,
        data,
        resync,
    } = slot;
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let runtime = match host.runtime(plugin, context) {
            Ok(runtime) => runtime,
            Err(error) => return ArtifactState::Failed(error),
        };
        if let Some(error) = runtime.error.clone() {
            return ArtifactState::Failed(error);
        }
        let messages = runtime.instances.report_artifact(
            instance,
            context,
            &client,
            client_id,
            block_types,
            block,
            data,
            resync,
        );
        runtime.send(messages);
        match runtime.instances.artifact_description(instance) {
            Some(ArtifactDescription::Described { source, summary }) => ArtifactState::Described {
                source: Uuid::from_bytes(source),
                summary,
            },
            Some(ArtifactDescription::Unreadable(error)) => ArtifactState::Failed(error),
            None => ArtifactState::Starting,
        }
    })
}

pub(crate) fn preview(painter: &egui::Painter, slot: PreviewSlot<'_>) -> PreviewPresentation {
    let PreviewSlot {
        plugin,
        block_types,
        client,
        client_id,
        block_id,
        block_type,
        instance,
        corners,
        opacity,
    } = slot;
    let context = painter.ctx().clone();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let Ok(runtime) = host.runtime(plugin, &context) else {
            return PreviewPresentation::empty();
        };
        if runtime.error.is_some() {
            return PreviewPresentation::empty();
        }
        let rect = egui::Rect::from_points(&corners);
        let scale_factor = context.pixels_per_point();
        let size = preview_size(rect.size(), scale_factor);
        let cropped = Quad {
            rect,
            corners,
            opacity,
        }
        .crop_to(painter.clip_rect());
        let visible = cropped
            .as_ref()
            .map_or(egui::Rect::ZERO, |(_, source)| scale_rect(*source, size));
        let pass = runtime.pass;
        let screen = runtime.instances.report(
            instance,
            EditorRegion::Preview,
            &context,
            &client,
            client_id,
            InstanceRole::Editor(EditorBlock {
                id: block_id,
                block_type,
            }),
            block_types,
            None,
            size,
            visible,
            scale_factor,
            pass,
        );
        let (children, _) = runtime.instances.host_children(
            instance,
            EditorRegion::Preview,
            egui::Rect::from_min_size(egui::Pos2::ZERO, size),
            egui::Rect::EVERYTHING,
        );
        let Some((quad, _)) = cropped else {
            return PreviewPresentation {
                drawn: false,
                size,
                children,
            };
        };
        let placed = runtime.layout.placement(screen).is_some();
        if placed {
            painter.add(runtime.present(
                quad.rect.intersect(painter.clip_rect()),
                screen,
                quad,
                UNIT,
            ));
        }
        PreviewPresentation {
            drawn: placed,
            size,
            children,
        }
    })
}

impl PreviewPresentation {
    fn empty() -> Self {
        Self {
            drawn: false,
            size: egui::Vec2::ZERO,
            children: Vec::new(),
        }
    }
}

pub(crate) fn poll(context: &egui::Context, frame: &eframe::Frame) {
    let pass = context.cumulative_pass_nr();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        for runtime in host.runtimes.values_mut() {
            runtime.detect_error();
            runtime.pump();
            runtime.begin_frame(frame, pass);
        }
        let grabbed = host
            .runtimes
            .values()
            .any(|runtime| runtime.instances.grabbing());
        if host.grabbed != grabbed {
            host.grabbed = grabbed;
            let grab = match grabbed {
                true => egui::CursorGrab::Locked,
                false => egui::CursorGrab::None,
            };
            context.send_viewport_cmd(egui::ViewportCommand::CursorGrab(grab));
            context.send_viewport_cmd(egui::ViewportCommand::CursorVisible(!grabbed));
        }
    });
}

pub(crate) fn flush() {
    HOST.with(|host| {
        for runtime in host.borrow_mut().runtimes.values_mut() {
            runtime.flush();
        }
    });
}

pub(crate) fn kill(context: &egui::Context, plugin_id: &str) {
    HOST.with(|host| {
        host.borrow_mut().shutdown(context, plugin_id);
    });
}

pub(crate) fn close(_context: &egui::Context, plugin_id: &str, instance: EditorInstanceId) {
    with(plugin_id, |runtime| {
        let messages = runtime
            .instances
            .remove(instance)
            .then(|| vec![Message::Editor(EditorMessage::Close { instance })]);
        if let Some(messages) = messages {
            runtime.send(messages);
        }
    });
}

pub(crate) fn artifact_draft(plugin_id: &str, instance: EditorInstanceId) -> Option<Vec<u8>> {
    with(plugin_id, |runtime| {
        runtime.instances.artifact_draft(instance)
    })
    .flatten()
}

pub(crate) fn regenerate_artifact(plugin_id: &str, instance: EditorInstanceId, data: &[u8]) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.regenerate_artifact(instance, data);
        runtime.send(messages);
    });
}

pub(crate) fn take_artifact_outcome(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Option<Result<(), String>> {
    with(plugin_id, |runtime| {
        runtime.instances.take_artifact_outcome(instance)
    })
    .flatten()
}

pub(crate) fn region_size(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
) -> Option<egui::Vec2> {
    with(plugin_id, |runtime| {
        runtime.instances.region_size(instance, region)
    })
    .flatten()
}

pub(crate) fn creation_ready(plugin_id: &str, instance: EditorInstanceId) -> bool {
    with(plugin_id, |runtime| {
        runtime.instances.creation_ready(instance)
    })
    .unwrap_or_default()
}

pub(crate) fn commit_creation(plugin_id: &str, instance: EditorInstanceId) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.commit_creation(instance);
        runtime.send(messages);
    });
}

pub(crate) fn take_created(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Option<Result<Uuid, String>> {
    with(plugin_id, |runtime| {
        runtime.instances.take_created(instance)
    })
    .flatten()
}

pub(crate) fn frame_child(plugin_id: &str, instance: EditorInstanceId) -> Option<Uuid> {
    with(plugin_id, |runtime| runtime.instances.frame_child(instance)).flatten()
}

pub(crate) fn revoke_frame_child(plugin_id: &str, instance: EditorInstanceId) {
    with(plugin_id, |runtime| {
        runtime.instances.revoke_frame_child(instance);
    });
}

pub(crate) fn take_leaving(plugin_id: &str, instance: EditorInstanceId) -> bool {
    with(plugin_id, |runtime| {
        runtime.instances.take_leaving(instance)
    })
    .unwrap_or_default()
}

pub(crate) fn frame_rects(plugin_id: &str, instance: EditorInstanceId) -> Option<HostFrame> {
    with(plugin_id, |runtime| {
        runtime.instances.frame_report(instance).map(|report| {
            let rect = |rect: &block_plugin_api::ChildRect| {
                egui::Rect::from_min_size(
                    egui::pos2(rect.x, rect.y),
                    egui::vec2(rect.width, rect.height),
                )
            };
            HostFrame {
                content: rect(&report.content),
            }
        })
    })
    .flatten()
}

pub(crate) fn presenting(plugin_id: &str, instance: EditorInstanceId) -> bool {
    with(plugin_id, |runtime| runtime.instances.presenting(instance)).unwrap_or_default()
}

pub(crate) fn present(
    context: &egui::Context,
    plugin_id: &str,
    instance: EditorInstanceId,
    presenting: bool,
) {
    with(plugin_id, |runtime| {
        if runtime.instances.set_presenting(instance, presenting) {
            context.request_repaint();
        }
    });
}

pub(crate) fn resized(plugin_id: &str, instance: EditorInstanceId, size: egui::Vec2) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.resized(instance, size);
        runtime.send(messages);
    });
}

pub(crate) fn take_view_changes(plugin_id: &str, instance: EditorInstanceId) -> Vec<ViewChange> {
    with(plugin_id, |runtime| {
        runtime.instances.take_view_changes(instance)
    })
    .unwrap_or_default()
}

pub(crate) fn aspect_ratio(plugin_id: &str, instance: EditorInstanceId) -> Option<f32> {
    with(plugin_id, |runtime| {
        runtime.instances.aspect_ratio(instance)
    })
    .flatten()
}

pub(crate) fn intrinsic_size(plugin_id: &str, instance: EditorInstanceId) -> Option<egui::Vec2> {
    with(plugin_id, |runtime| {
        runtime.instances.intrinsic_size(instance)
    })
    .flatten()
}

pub(crate) fn running() -> Vec<RuntimeStatus> {
    HOST.with(|host| {
        let host = host.borrow();
        let mut running: Vec<_> = host
            .runtimes
            .iter()
            .map(|(plugin_id, runtime)| RuntimeStatus {
                plugin_id: plugin_id.clone(),
                state: runtime.state(),
                surface: SurfaceStatus {
                    index: runtime.surface,
                    generation: runtime.layout.generation,
                    width: runtime.layout.width,
                    height: runtime.layout.height,
                    placements: runtime.layout.screens.len(),
                },
                pass: runtime.pass,
                uptime: runtime.backend.uptime(),
                instances: runtime.instances.statuses(&runtime.layout, runtime.pass),
            })
            .collect();
        running.sort_by(|left, right| left.plugin_id.cmp(&right.plugin_id));
        running
    })
}

pub(super) fn with<R>(plugin_id: &str, act: impl FnOnce(&mut Runtime) -> R) -> Option<R> {
    HOST.with(|host| Some(act(host.borrow_mut().runtimes.get_mut(plugin_id)?)))
}

#[cfg(test)]
mod tests;
