use block_client::BlockClient;
use block_plugin_api::{
    ArtifactDescription, AssetResult, BlockPick, ChildId, ChildPlacement, ChildPlacements,
    ChildRect, ChildStatus, CreationOutcome, CursorIcon, EditorBand, EditorInstanceId,
    EditorMessage, EditorRegion, FetchResult, FilePick, FrameChrome, FrameReport, FrameSpec,
    InputEvent, Message, Occluder, PointerButton, RegionSize, ScreenPlacement, ScreenRequest,
    ViewportMetrics, WheelUnit, MAX_CHILDREN, MAX_COLLECTION_ITEMS,
};
use block_ui::BlockCatalog;
use eframe::egui;
use std::{collections::HashMap, rc::Rc, sync::Arc};
use uuid::Uuid;

use crate::{host::BlockDrag, EditorHost, Waker};

pub(crate) struct EguiSession {
    app: Box<dyn AppUi>,
    chrome: Rc<Vec<EditorBand>>,
    instance: EditorInstanceId,
    regions: HashMap<EditorRegion, RegionState>,
    host: EditorHost,
    drag: Option<(EditorRegion, BlockDrag)>,
    files: Option<(EditorRegion, crate::host::FileDrop)>,
    intrinsic: Option<egui::Vec2>,
    aspect_ratio: Option<f32>,
    creating: bool,
    leaving: bool,
    created: Option<CreationOutcome>,
    artifact: Option<ArtifactState>,
    generation: u64,
}

struct ArtifactState {
    data: Vec<u8>,
    draft: Vec<u8>,
    described: Option<Vec<u8>>,
    edited: bool,
    regenerating: bool,
}

impl ArtifactState {
    fn new(data: Vec<u8>) -> Self {
        Self {
            draft: data.clone(),
            data,
            described: None,
            edited: false,
            regenerating: false,
        }
    }

    fn settings(&mut self, data: Vec<u8>) {
        self.draft.clone_from(&data);
        self.data = data;
        self.edited = false;
    }
}

#[derive(Default)]
struct RegionState {
    input: egui::RawInput,
    placement: Option<ScreenPlacement>,
    metrics: Option<ViewportMetrics>,
    frame: Option<FrameSpec>,
    used: Option<egui::Vec2>,
    reported: Option<egui::Vec2>,
    report: Option<FrameReport>,
    reported_frame: Option<FrameReport>,
    cursor: CursorIcon,
    reported_cursor: Option<CursorIcon>,
    children: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
    reported_children: Option<(Vec<ChildPlacement>, Vec<Occluder>)>,
}

trait AppUi {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid);
    fn connect_creation(&mut self, host: EditorHost, client: Arc<BlockClient>);
    fn create_block(&mut self) -> Result<Uuid, String>;
    fn creation_ui(&mut self, ui: &mut egui::Ui);
    fn connect_artifact(
        &mut self,
        host: EditorHost,
        client: Arc<BlockClient>,
        artifact: crate::Artifact,
    );
    fn describe_artifact(&mut self, data: &[u8]) -> ArtifactDescription;
    fn artifact_settings_ui(&mut self, ui: &mut egui::Ui, data: &mut Vec<u8>);
    fn regenerate_artifact(&mut self, data: &[u8]);
    fn poll_artifact(&mut self) -> Option<Result<(), String>>;
    fn main_ui(&mut self, ui: &mut egui::Ui);
    fn toolbar_ui(&mut self, ui: &mut egui::Ui);
    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui);
    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui);
    fn preview_ui(&mut self, ui: &mut egui::Ui);
    fn intrinsic_size(&mut self) -> Option<egui::Vec2>;
    fn set_intrinsic_size(&mut self, size: egui::Vec2);
    fn aspect_ratio(&mut self) -> Option<f32>;
}

impl<A: crate::App> AppUi for A {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        crate::App::connect(self, host, client, block_id);
    }

    fn connect_creation(&mut self, host: EditorHost, client: Arc<BlockClient>) {
        crate::App::connect_creation(self, host, client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        crate::App::create_block(self)
    }

    fn creation_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::creation_ui(self, ui);
    }

    fn connect_artifact(
        &mut self,
        host: EditorHost,
        client: Arc<BlockClient>,
        artifact: crate::Artifact,
    ) {
        crate::App::connect_artifact(self, host, client, artifact);
    }

    fn describe_artifact(&mut self, data: &[u8]) -> ArtifactDescription {
        match crate::App::describe_artifact(self, data) {
            Ok(description) => ArtifactDescription::Described {
                source: description.source.into_bytes(),
                summary: description.summary,
            },
            Err(error) => ArtifactDescription::Unreadable(error),
        }
    }

    fn artifact_settings_ui(&mut self, ui: &mut egui::Ui, data: &mut Vec<u8>) {
        crate::App::artifact_settings_ui(self, ui, data);
    }

    fn regenerate_artifact(&mut self, data: &[u8]) {
        crate::App::regenerate_artifact(self, data);
    }

    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        crate::App::poll_artifact(self)
    }

    fn main_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::ui(self, ui);
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::toolbar_ui(self, ui);
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::left_sidebar_ui(self, ui);
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::right_sidebar_ui(self, ui);
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::preview_ui(self, ui);
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        crate::App::intrinsic_size(self)
    }

    fn set_intrinsic_size(&mut self, size: egui::Vec2) {
        crate::App::set_intrinsic_size(self, size);
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        crate::App::aspect_ratio(self)
    }
}

impl EguiSession {
    pub(crate) fn new<A: crate::App>(
        chrome: Rc<Vec<EditorBand>>,
        instance: EditorInstanceId,
        waker: Waker,
    ) -> Self {
        Self {
            app: Box::new(A::default()),
            chrome,
            instance,
            regions: HashMap::new(),
            host: EditorHost::new(waker),
            drag: None,
            files: None,
            intrinsic: None,
            aspect_ratio: None,
            creating: false,
            leaving: false,
            created: None,
            artifact: None,
            generation: 0,
        }
    }

    pub(crate) fn set_block_types(&self, catalog: Rc<BlockCatalog>) {
        self.host.set_block_types(catalog);
    }

    pub(crate) fn set_pasted_image(&self, request: u64, image: block_plugin_api::ClipboardImage) {
        self.host.set_pasted_image(request, image);
    }

    pub(crate) fn set_audio(&self, status: block_plugin_api::AudioStatus) {
        self.host.set_audio(status);
    }

    pub(crate) fn set_client_id(&self, client_id: Uuid) {
        self.host.set_client_id(client_id);
    }

    pub(crate) fn set_editable(&self, editable: bool) {
        self.host.set_editable(editable);
    }

    pub(crate) fn set_view(&self, view: egui::Rect) {
        self.host.set_view(view);
    }

    pub(crate) fn resized(&mut self, size: egui::Vec2) {
        self.app.set_intrinsic_size(size);
    }

    pub(crate) fn set_presenting(&self, presenting: bool) {
        self.host.set_presenting(presenting);
    }

    pub(crate) fn set_drag(&mut self, drag: Option<(EditorRegion, BlockDrag)>) {
        self.drag = drag;
    }

    pub(crate) fn set_files(&mut self, files: Option<(EditorRegion, crate::host::FileDrop)>) {
        self.files = files;
    }

    pub(crate) fn connect(&mut self, client: Arc<BlockClient>, block_id: Uuid) {
        self.app.connect(self.host.clone(), client, block_id);
    }

    pub(crate) fn connect_creation(&mut self, client: Arc<BlockClient>) {
        self.creating = true;
        self.host.set_editable(true);
        self.app.connect_creation(self.host.clone(), client);
    }

    pub(crate) fn connect_artifact(
        &mut self,
        client: Arc<BlockClient>,
        block_id: Uuid,
        block_type: Uuid,
        data: Vec<u8>,
    ) {
        self.artifact = Some(ArtifactState::new(data));
        self.host.set_editable(true);
        self.app.connect_artifact(
            self.host.clone(),
            client,
            crate::Artifact {
                block_id,
                block_type,
            },
        );
    }

    pub(crate) fn artifact_settings(&mut self, data: Vec<u8>) {
        if let Some(artifact) = &mut self.artifact {
            artifact.settings(data);
        }
    }

    pub(crate) fn regenerate_artifact(&mut self, data: &[u8]) {
        let Some(artifact) = &mut self.artifact else {
            return;
        };
        artifact.regenerating = true;
        self.app.regenerate_artifact(data);
    }

    pub(crate) fn commit_creation(&mut self) {
        self.created = Some(match self.app.create_block() {
            Ok(block_id) => CreationOutcome::Created(block_id.into_bytes()),
            Err(error) => CreationOutcome::Failed(error),
        });
    }

    pub(crate) fn place(&mut self, placements: &[ScreenPlacement], requests: &[ScreenRequest]) {
        self.regions
            .retain(|region, _| placements.iter().any(|it| it.region == *region));
        for placement in placements {
            let Some(request) = requests
                .iter()
                .find(|request| request.screen == placement.screen)
            else {
                continue;
            };
            let state = self.regions.entry(placement.region).or_default();
            state.placement = Some(*placement);
            state.metrics = Some(request.metrics.clone());
            state.frame = request.frame.clone();
        }
    }

    pub(crate) fn outbound(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        let sizes = self.region_sizes();
        if !sizes.is_empty() {
            messages.push(Message::RegionSizes(sizes));
        }
        let frames = self.frame_reports();
        if !frames.is_empty() {
            messages.push(Message::Frames(frames));
        }
        let instance = self.instance;
        for (group, measurements) in self.host.take_performance() {
            messages.push(Message::Editor(EditorMessage::Performance {
                instance,
                group,
                measurements,
            }));
        }
        for (region, cursor) in self.cursors() {
            messages.push(Message::Editor(EditorMessage::Cursor {
                instance,
                region,
                cursor,
            }));
        }
        if let Some(accepted) = self.host.take_drag_accepted() {
            messages.push(Message::Editor(EditorMessage::DragAccepted {
                instance,
                accepted,
            }));
        }
        let intrinsic = self.app.intrinsic_size();
        if intrinsic != self.intrinsic {
            self.intrinsic = intrinsic;
            if let Some(size) = intrinsic {
                messages.push(Message::Editor(EditorMessage::IntrinsicSize {
                    instance,
                    width: size.x,
                    height: size.y,
                }));
            }
        }
        let aspect_ratio = self.app.aspect_ratio();
        if aspect_ratio != self.aspect_ratio {
            self.aspect_ratio = aspect_ratio;
            if let Some(ratio) = aspect_ratio {
                messages.push(Message::Editor(EditorMessage::AspectRatio {
                    instance,
                    ratio,
                }));
            }
        }
        if let Some(ready) = self.host.take_creation_ready() {
            messages.push(Message::Editor(EditorMessage::CreationReady {
                instance,
                ready,
            }));
        }
        if let Some(artifact) = &mut self.artifact {
            if artifact.described.as_deref() != Some(artifact.data.as_slice()) {
                artifact.described = Some(artifact.data.clone());
                let description = self.app.describe_artifact(&artifact.data);
                messages.push(Message::Editor(EditorMessage::ArtifactDescribed {
                    instance,
                    description,
                }));
            }
            if artifact.edited {
                artifact.edited = false;
                messages.push(Message::Editor(EditorMessage::ArtifactEdited {
                    instance,
                    data: artifact.draft.clone(),
                }));
            }
            if artifact.regenerating {
                if let Some(result) = self.app.poll_artifact() {
                    artifact.regenerating = false;
                    messages.push(Message::Editor(EditorMessage::ArtifactRegenerated {
                        instance,
                        outcome: match result {
                            Ok(()) => block_plugin_api::RegenerationOutcome::Done,
                            Err(error) => block_plugin_api::RegenerationOutcome::Failed(error),
                        },
                    }));
                }
            }
        }
        if std::mem::take(&mut self.leaving) {
            messages.push(Message::Editor(EditorMessage::LeaveFrame { instance }));
        }
        if let Some(outcome) = self.created.take() {
            messages.push(Message::Editor(EditorMessage::CreationBlock {
                instance,
                outcome,
            }));
        }
        for (request_id, filter) in self.host.take_picks() {
            messages.push(Message::Editor(EditorMessage::PickFile {
                instance,
                request_id,
                filter,
            }));
        }
        for (request_id, filter) in self.host.take_block_picks() {
            messages.push(Message::Editor(EditorMessage::PickBlock {
                instance,
                request_id,
                filter,
            }));
        }
        for request_id in self.host.take_pastes() {
            messages.push(Message::Editor(EditorMessage::PasteImage {
                instance,
                request_id,
            }));
        }
        for (block_id, command) in self.host.take_audio_commands() {
            messages.push(Message::Editor(EditorMessage::PlayAudio {
                instance,
                block_id: block_id.into_bytes(),
                command,
            }));
        }
        for (request_id, url) in self.host.take_fetches() {
            messages.push(Message::Editor(EditorMessage::Fetch {
                instance,
                request_id,
                url,
            }));
        }
        for (request_id, name) in self.host.take_asset_reads() {
            messages.push(Message::Editor(EditorMessage::ReadAsset {
                instance,
                request_id,
                name,
            }));
        }
        if let Some(grabbed) = self.host.take_cursor_grab() {
            messages.push(Message::Editor(EditorMessage::GrabCursor {
                instance,
                grabbed,
            }));
        }
        for placements in self.children() {
            messages.push(Message::Children(placements));
        }
        self.retain_child_statuses();
        for presenting in self.host.take_present_requests() {
            messages.push(Message::Editor(EditorMessage::Present {
                instance,
                presenting,
            }));
        }
        for change in self.host.take_view_changes() {
            messages.push(Message::Editor(EditorMessage::ChangeView {
                instance,
                change,
            }));
        }
        for (block_id, block_type) in self.host.take_opens() {
            messages.push(Message::Editor(EditorMessage::OpenBlock {
                instance,
                block_id: block_id.into_bytes(),
                block_type: block_type.into_bytes(),
            }));
        }
        messages
    }

    fn cursors(&mut self) -> Vec<(EditorRegion, CursorIcon)> {
        let mut cursors = Vec::new();
        for (region, state) in &mut self.regions {
            if state.reported_cursor == Some(state.cursor) {
                continue;
            }
            state.reported_cursor = Some(state.cursor);
            cursors.push((*region, state.cursor));
        }
        cursors
    }

    fn frame_reports(&mut self) -> Vec<FrameReport> {
        let mut reports = Vec::new();
        for state in self.regions.values_mut() {
            let Some(report) = &state.report else {
                continue;
            };
            if state.reported_frame.as_ref() == Some(report) {
                continue;
            }
            state.reported_frame = Some(report.clone());
            reports.push(report.clone());
        }
        reports
    }

    fn region_sizes(&mut self) -> Vec<RegionSize> {
        let mut sizes = Vec::new();
        for state in self.regions.values_mut() {
            let (Some(placement), Some(used)) = (state.placement, state.used) else {
                continue;
            };
            if state.reported == Some(used) {
                continue;
            }
            state.reported = Some(used);
            sizes.push(RegionSize {
                screen: placement.screen,
                logical_width: used.x,
                logical_height: used.y,
            });
        }
        sizes
    }

    fn used(&mut self, region: EditorRegion, content: egui::Rect) {
        let origin = self.rect(region).min;
        let Some(state) = self.regions.get_mut(&region) else {
            return;
        };
        let used = (content.max - origin).max(egui::Vec2::ZERO);
        state.used = Some(egui::vec2(used.x.round(), used.y.round()));
    }

    pub(crate) fn file_picked(&self, request_id: u64, pick: FilePick) {
        self.host.set_pick(request_id, pick);
    }

    pub(crate) fn block_picked(&self, request_id: u64, pick: BlockPick) {
        self.host.set_block_pick(request_id, pick);
    }

    pub(crate) fn fetched(&self, request_id: u64, result: FetchResult) {
        self.host.set_fetched(request_id, result);
    }

    pub(crate) fn asset_read(&self, request_id: u64, result: AssetResult) {
        self.host.set_asset(request_id, result);
    }

    pub(crate) fn set_child_statuses(&self, statuses: Vec<ChildStatus>) {
        self.host.set_child_statuses(statuses);
    }

    fn children(&mut self) -> Vec<ChildPlacements> {
        let instance = self.instance;
        let generation = self.generation;
        let mut messages = Vec::new();
        for (region, state) in &mut self.regions {
            let current = bounded(&state.children, &state.occluders);
            if state.reported_children.as_ref() == Some(&current) {
                continue;
            }
            state.reported_children = Some(current.clone());
            let (children, occluders) = current;
            messages.push(ChildPlacements {
                instance,
                region: *region,
                generation,
                children,
                occluders,
            });
        }
        messages
    }

    fn retain_child_statuses(&self) {
        let live: Vec<ChildId> = self
            .regions
            .values()
            .flat_map(|state| state.children.iter().map(|child| child.child))
            .collect();
        self.host.retain_child_statuses(&live);
    }

    pub(crate) fn run(
        &mut self,
        region: EditorRegion,
        context: &egui::Context,
        time: f64,
        generation: u64,
    ) -> egui::FullOutput {
        self.generation = generation;
        let scale_factor = self.scale_factor(region);
        let rect = self.rect(region);
        let visible_rect = self.visible_rect(region);
        let state = self.regions.entry(region).or_default();
        state.input.screen_rect = Some(rect);
        state.input.time = Some(time);
        let mut input = std::mem::take(&mut state.input);
        input.viewport_id = viewport_id(region);
        input
            .viewports
            .entry(input.viewport_id)
            .or_default()
            .native_pixels_per_point = Some(scale_factor);
        state.input.focused = input.focused;
        state.input.modifiers = input.modifiers;
        let drag = self.drag.and_then(|(dragged, drag)| {
            (dragged == region).then(|| BlockDrag {
                position: drag.position + rect.min.to_vec2(),
                ..drag
            })
        });
        self.host.set_drag(drag);
        let files = self.files.as_ref().and_then(|(dropped, files)| {
            (*dropped == region).then(|| crate::host::FileDrop {
                position: files.position + rect.min.to_vec2(),
                ..files.clone()
            })
        });
        let delivered_files = files.as_ref().is_some_and(|files| files.dropped);
        self.host.set_files(files);
        let delivered_drop = drag.is_some_and(|drag| drag.dropped);
        let app = &mut self.app;
        let plain =
            egui::Frame::central_panel(&context.global_style()).inner_margin(egui::Margin::ZERO);
        let creating = self.creating;
        let artifact = &mut self.artifact;
        let mut draft = artifact
            .as_mut()
            .filter(|_| region == EditorRegion::ArtifactSettings)
            .map(|artifact| artifact.draft.clone());
        let spec = self
            .regions
            .get(&region)
            .and_then(|state| state.frame.clone())
            .unwrap_or_default();
        let chrome = Rc::clone(&self.chrome);
        let host = self.host.clone();
        self.host.begin_region(region, rect.min.to_vec2());
        let mut content = rect;
        let mut painted = Vec::new();
        let mut leaving = false;
        let mut content_band = rect;
        let output = context.run_ui(input, |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(visible_rect));
            match (&mut draft, creating, region) {
                (Some(draft), _, _) => {
                    egui::CentralPanel::default()
                        .frame(plain)
                        .show_inside(ui, |ui| app.artifact_settings_ui(ui, draft));
                }
                (None, true, _) => {
                    egui::CentralPanel::default()
                        .frame(plain)
                        .show_inside(ui, |ui| app.creation_ui(ui));
                }
                (None, false, EditorRegion::Frame) => {
                    let band = |band| chrome.contains(&band) && host.band_shown(band);
                    let mut bands = AppBands {
                        app,
                        background: plain.fill,
                    };
                    let outcome = block_ui::frame::Frame::new(egui::Id::new("plugin frame"))
                        .chrome(match spec.chrome {
                            FrameChrome::Drawn => block_ui::frame::Chrome::Drawn,
                            FrameChrome::Reserved => block_ui::frame::Chrome::Reserved,
                            FrameChrome::None => block_ui::frame::Chrome::None,
                        })
                        .toolbar(band(EditorBand::Toolbar))
                        .left_sidebar(band(EditorBand::LeftSidebar))
                        .right_sidebar(band(EditorBand::RightSidebar))
                        .content(
                            spec.content
                                .map(|content| host_rect(content, rect.min.to_vec2())),
                        )
                        .trail(spec.trail.clone())
                        .show(ui, &mut bands);
                    leaving |= outcome.exit;
                    content_band = outcome.rects.content;
                    painted = outcome.rects.painted().collect();
                }
                (None, false, EditorRegion::Preview) => {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show_inside(ui, |ui| app.preview_ui(ui));
                }
                (None, false, EditorRegion::ArtifactSettings) => {}
            }
            content = ui.min_rect();
        });
        if let (Some(artifact), Some(draft)) = (artifact.as_mut(), draft) {
            artifact.edited |= artifact.draft != draft;
            artifact.draft = draft;
        }
        let floating = floating_rects(context, visible_rect);
        for occluder in &floating {
            self.host.occlude(*occluder);
        }
        let (children, occluders) = self.host.end_region(region);
        let origin = rect.min.to_vec2();
        let screen = self.placement(region).map(|placement| placement.screen);
        if let (Some(state), Some(screen)) = (self.regions.get_mut(&region), screen) {
            state.children = children;
            state.occluders = occluders;
            state.report = (region == EditorRegion::Frame).then(|| FrameReport {
                screen,
                content: plugin_rect(content_band, origin),
                painted: painted
                    .iter()
                    .map(|rect| plugin_rect(*rect, origin))
                    .collect(),
                floating: floating
                    .iter()
                    .map(|rect| plugin_rect(*rect, origin))
                    .collect(),
            });
        }
        self.host.set_drag(None);
        self.host.set_files(None);
        if delivered_drop {
            self.drag = None;
        }
        if delivered_files {
            self.files = None;
        }
        self.leaving |= leaving;
        self.used(region, content);
        let cursor = cursor_icon(output.platform_output.cursor_icon);
        if let Some(state) = self.regions.get_mut(&region) {
            state.cursor = cursor;
        }
        output
    }

    pub(crate) fn scale_factor(&self, region: EditorRegion) -> f32 {
        self.placement(region)
            .map_or(1.0, |placement| placement.scale_factor())
    }

    fn placement(&self, region: EditorRegion) -> Option<&ScreenPlacement> {
        self.regions
            .get(&region)
            .and_then(|state| state.placement.as_ref())
    }

    pub(crate) fn visible_rect(&self, region: EditorRegion) -> egui::Rect {
        let Some(placement) = self.placement(region) else {
            return egui::Rect::ZERO;
        };
        let scale = placement.scale_factor();
        egui::Rect::from_min_size(
            egui::pos2(placement.x as f32 / scale, placement.y as f32 / scale),
            egui::vec2(
                placement.width as f32 / scale,
                placement.height as f32 / scale,
            ),
        )
    }

    fn rect(&self, region: EditorRegion) -> egui::Rect {
        let state = self.regions.get(&region);
        let (Some(placement), Some(metrics)) = (
            state.and_then(|state| state.placement.as_ref()),
            state.and_then(|state| state.metrics.as_ref()),
        ) else {
            return egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO);
        };
        let scale = placement.scale_factor();
        egui::Rect::from_min_size(
            egui::pos2(
                placement.x as f32 / scale - metrics.visible_x,
                placement.y as f32 / scale - metrics.visible_y,
            ),
            egui::vec2(metrics.logical_width, metrics.logical_height),
        )
    }

    pub(crate) fn input(&mut self, region: EditorRegion, event: &InputEvent) {
        let origin = self.rect(region).min.to_vec2();
        let Some(state) = self.regions.get_mut(&region) else {
            return;
        };
        match event {
            InputEvent::PointerMoved { x, y } => state
                .input
                .events
                .push(egui::Event::PointerMoved(egui::pos2(*x, *y) + origin)),
            InputEvent::PointerMotion { x, y } => state
                .input
                .events
                .push(egui::Event::MouseMoved(egui::vec2(*x, *y))),
            InputEvent::PointerButton {
                button,
                pressed,
                x,
                y,
            } => {
                state.input.events.push(egui::Event::PointerButton {
                    pos: egui::pos2(*x, *y) + origin,
                    button: pointer_button(*button),
                    pressed: *pressed,
                    modifiers: state.input.modifiers,
                });
            }
            InputEvent::Wheel { x, y, unit } => {
                state.input.events.push(egui::Event::MouseWheel {
                    unit: wheel_unit(*unit),
                    delta: egui::vec2(*x, *y),
                    phase: egui::TouchPhase::Move,
                    modifiers: state.input.modifiers,
                });
            }
            InputEvent::Zoom { factor } => state.input.events.push(egui::Event::Zoom(*factor)),
            InputEvent::Key {
                logical,
                pressed,
                repeat,
                ..
            } => {
                if let Some(key) = egui::Key::from_name(logical) {
                    state.input.events.push(egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: *pressed,
                        repeat: *repeat,
                        modifiers: state.input.modifiers,
                    });
                }
            }
            InputEvent::Text(text) => state.input.events.push(egui::Event::Text(text.clone())),
            InputEvent::Paste(text) => state.input.events.push(egui::Event::Paste(text.clone())),
            InputEvent::Modifiers(modifiers) => {
                state.input.modifiers = egui::Modifiers {
                    alt: modifiers.alt,
                    ctrl: modifiers.control,
                    shift: modifiers.shift,
                    mac_cmd: false,
                    command: modifiers.command,
                };
            }
            InputEvent::Focus(focused) => state.input.focused = *focused,
        }
    }
}

struct AppBands<'a> {
    app: &'a mut Box<dyn AppUi>,
    background: egui::Color32,
}

impl block_ui::frame::FrameBands for AppBands<'_> {
    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        self.app.toolbar_ui(ui);
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.app.left_sidebar_ui(ui);
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.app.right_sidebar_ui(ui);
    }

    fn content_ui(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, self.background);
        self.app.main_ui(ui);
    }
}

fn host_rect(rect: ChildRect, origin: egui::Vec2) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(rect.x, rect.y) + origin,
        egui::vec2(rect.width, rect.height),
    )
}

fn plugin_rect(rect: egui::Rect, origin: egui::Vec2) -> ChildRect {
    let rect = rect.translate(-origin);
    ChildRect {
        x: rect.min.x,
        y: rect.min.y,
        width: rect.width(),
        height: rect.height(),
    }
}

pub(crate) fn viewport_id(region: EditorRegion) -> egui::ViewportId {
    egui::ViewportId(egui::Id::new(("plugin editor region", region)))
}

fn bounded(
    children: &[ChildPlacement],
    occluders: &[Occluder],
) -> (Vec<ChildPlacement>, Vec<Occluder>) {
    let kept = children.len().min(MAX_CHILDREN);
    (
        children[..kept].to_vec(),
        occluders
            .iter()
            .take(MAX_COLLECTION_ITEMS)
            .map(|occluder| Occluder {
                after: occluder.after.min(kept as u32),
                rect: occluder.rect,
            })
            .collect(),
    )
}

fn floating_rects(context: &egui::Context, visible: egui::Rect) -> Vec<egui::Rect> {
    context.memory(|memory| {
        memory
            .layer_ids()
            .filter(|layer| layer.order >= egui::Order::Middle)
            .filter_map(|layer| memory.area_rect(layer.id))
            .filter(|rect| rect.intersects(visible))
            .collect()
    })
}

fn cursor_icon(cursor: egui::CursorIcon) -> CursorIcon {
    match cursor {
        egui::CursorIcon::None => CursorIcon::None,
        egui::CursorIcon::PointingHand => CursorIcon::Pointer,
        egui::CursorIcon::Text | egui::CursorIcon::VerticalText => CursorIcon::Text,
        egui::CursorIcon::Crosshair | egui::CursorIcon::Cell => CursorIcon::Crosshair,
        egui::CursorIcon::Grab => CursorIcon::Grab,
        egui::CursorIcon::Grabbing => CursorIcon::Grabbing,
        egui::CursorIcon::Move | egui::CursorIcon::AllScroll => CursorIcon::Move,
        egui::CursorIcon::NotAllowed | egui::CursorIcon::NoDrop => CursorIcon::NotAllowed,
        egui::CursorIcon::Wait => CursorIcon::Wait,
        egui::CursorIcon::Progress => CursorIcon::Progress,
        egui::CursorIcon::Help => CursorIcon::Help,
        egui::CursorIcon::ResizeHorizontal
        | egui::CursorIcon::ResizeColumn
        | egui::CursorIcon::ResizeEast
        | egui::CursorIcon::ResizeWest => CursorIcon::ResizeHorizontal,
        egui::CursorIcon::ResizeVertical
        | egui::CursorIcon::ResizeRow
        | egui::CursorIcon::ResizeNorth
        | egui::CursorIcon::ResizeSouth => CursorIcon::ResizeVertical,
        egui::CursorIcon::ResizeNeSw
        | egui::CursorIcon::ResizeNorthEast
        | egui::CursorIcon::ResizeSouthWest => CursorIcon::ResizeNeSw,
        egui::CursorIcon::ResizeNwSe
        | egui::CursorIcon::ResizeNorthWest
        | egui::CursorIcon::ResizeSouthEast => CursorIcon::ResizeNwSe,
        _ => CursorIcon::Default,
    }
}

fn pointer_button(button: PointerButton) -> egui::PointerButton {
    match button {
        PointerButton::Primary => egui::PointerButton::Primary,
        PointerButton::Secondary => egui::PointerButton::Secondary,
        PointerButton::Middle => egui::PointerButton::Middle,
        PointerButton::Back => egui::PointerButton::Extra1,
        PointerButton::Forward | PointerButton::Other(_) => egui::PointerButton::Extra2,
    }
}

fn wheel_unit(unit: WheelUnit) -> egui::MouseWheelUnit {
    match unit {
        WheelUnit::Pixels => egui::MouseWheelUnit::Point,
        WheelUnit::Lines => egui::MouseWheelUnit::Line,
        WheelUnit::Pages => egui::MouseWheelUnit::Page,
    }
}
