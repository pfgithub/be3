use block_client::TunnelCarrier;
use block_plugin_api::{
    EditorInstanceId, EditorMessage, EditorRegion, FilePick, InputEvent, Message, PointerButton,
    RegionSize, ScreenPlacement, TunnelMessage, WheelUnit,
};
use block_ui::BlockCatalog;
use eframe::egui;
use std::{collections::HashMap, rc::Rc};
use uuid::Uuid;

use crate::{host::BlockDrag, EditorHost};

pub(crate) struct EguiSession {
    app: Box<dyn AppUi>,
    instance: EditorInstanceId,
    regions: HashMap<EditorRegion, RegionState>,
    carrier: Option<TunnelCarrier>,
    host: EditorHost,
    drag: Option<(EditorRegion, BlockDrag)>,
    intrinsic: Option<egui::Vec2>,
    aspect_ratio: Option<f32>,
    creating: bool,
}

#[derive(Default)]
struct RegionState {
    input: egui::RawInput,
    placement: Option<ScreenPlacement>,
    used: Option<egui::Vec2>,
    reported: Option<egui::Vec2>,
}

trait AppUi {
    fn connect(&mut self, host: EditorHost, client: block_client::BlockClient, block_id: Uuid);
    fn connect_creation(&mut self, host: EditorHost);
    fn creation_ui(&mut self, ui: &mut egui::Ui);
    fn ui(&mut self, ui: &mut egui::Ui, region: EditorRegion);
    fn intrinsic_size(&mut self) -> Option<egui::Vec2>;
    fn aspect_ratio(&mut self) -> Option<f32>;
}

impl<A: crate::App> AppUi for A {
    fn connect(&mut self, host: EditorHost, client: block_client::BlockClient, block_id: Uuid) {
        crate::App::connect(self, host, client, block_id);
    }

    fn connect_creation(&mut self, host: EditorHost) {
        crate::App::connect_creation(self, host);
    }

    fn creation_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::creation_ui(self, ui);
    }

    fn ui(&mut self, ui: &mut egui::Ui, region: EditorRegion) {
        match region {
            EditorRegion::Main => crate::App::ui(self, ui),
            EditorRegion::Toolbar => crate::App::toolbar_ui(self, ui),
            EditorRegion::LeftSidebar => crate::App::left_sidebar_ui(self, ui),
            EditorRegion::RightSidebar => crate::App::right_sidebar_ui(self, ui),
            EditorRegion::Preview => crate::App::preview_ui(self, ui),
        }
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        crate::App::intrinsic_size(self)
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        crate::App::aspect_ratio(self)
    }
}

impl EguiSession {
    pub(crate) fn new<A: crate::App>(instance: EditorInstanceId) -> Self {
        Self {
            app: Box::new(A::default()),
            instance,
            regions: HashMap::new(),
            carrier: None,
            host: EditorHost::default(),
            drag: None,
            intrinsic: None,
            aspect_ratio: None,
            creating: false,
        }
    }

    pub(crate) fn set_block_types(&self, catalog: Rc<BlockCatalog>) {
        self.host.set_block_types(catalog);
    }

    pub(crate) fn set_drag(&mut self, drag: Option<(EditorRegion, BlockDrag)>) {
        self.drag = drag;
    }

    pub(crate) fn connect(&mut self, block_id: Uuid, account_id: Uuid, workspace_id: Uuid) {
        if self.carrier.is_some() {
            return;
        }
        let (endpoint, carrier) = block_client::tunnel_channel();
        let client = block_client::BlockClient::tunneled(account_id, workspace_id, endpoint);
        self.app.connect(self.host.clone(), client, block_id);
        self.carrier = Some(carrier);
    }

    pub(crate) fn connect_creation(&mut self) {
        if self.creating {
            return;
        }
        self.creating = true;
        self.app.connect_creation(self.host.clone());
    }

    pub(crate) fn place(&mut self, placements: &[ScreenPlacement]) {
        self.regions
            .retain(|region, _| placements.iter().any(|it| it.region == *region));
        for placement in placements {
            self.regions.entry(placement.region).or_default().placement = Some(*placement);
        }
    }

    pub(crate) fn outbound(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        let sizes = self.region_sizes();
        if !sizes.is_empty() {
            messages.push(Message::RegionSizes(sizes));
        }
        let instance = self.instance;
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
        if let Some(payload) = self.host.take_proposal() {
            messages.push(Message::Editor(EditorMessage::CreationContent {
                instance,
                payload,
            }));
        }
        for (request_id, filter) in self.host.take_picks() {
            messages.push(Message::Editor(EditorMessage::PickFile {
                instance,
                request_id,
                filter,
            }));
        }
        for (block_id, block_type) in self.host.take_opens() {
            messages.push(Message::Editor(EditorMessage::OpenBlock {
                instance,
                block_id: block_id.into_bytes(),
                block_type: block_type.into_bytes(),
            }));
        }
        let Some(carrier) = &mut self.carrier else {
            return messages;
        };
        while let Some(payload) = carrier.try_recv() {
            messages.push(Message::Client(TunnelMessage::Request {
                instance,
                payload,
            }));
        }
        messages
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
        state.used = Some((content.max - origin).max(egui::Vec2::ZERO));
    }

    pub(crate) fn file_picked(&self, request_id: u64, pick: FilePick) {
        self.host.set_pick(request_id, pick);
    }

    pub(crate) fn client_message(&mut self, message: &TunnelMessage) {
        let Some(carrier) = &self.carrier else {
            return;
        };
        if let TunnelMessage::Response { payload, .. } = message {
            carrier.send(payload.clone());
        }
    }

    pub(crate) fn run(
        &mut self,
        region: EditorRegion,
        context: &egui::Context,
        time: f64,
    ) -> egui::FullOutput {
        context.set_pixels_per_point(self.scale_factor(region));
        let rect = self.rect(region);
        let state = self.regions.entry(region).or_default();
        state.input.screen_rect = Some(rect);
        state.input.time = Some(time);
        let input = std::mem::take(&mut state.input);
        state.input.focused = input.focused;
        state.input.modifiers = input.modifiers;
        let drag = self.drag.and_then(|(dragged, drag)| {
            (dragged == region).then(|| BlockDrag {
                position: drag.position + rect.min.to_vec2(),
                ..drag
            })
        });
        self.host.set_drag(drag);
        let delivered_drop = drag.is_some_and(|drag| drag.dropped);
        let app = &mut self.app;
        let frame = if region == EditorRegion::Preview {
            egui::Frame::NONE
        } else {
            egui::Frame::central_panel(&context.global_style()).inner_margin(egui::Margin::ZERO)
        };
        let creating = self.creating;
        let output = context.run_ui(input, |ui| {
            egui::CentralPanel::default().frame(frame).show_inside(ui, {
                |ui| {
                    if creating {
                        app.creation_ui(ui);
                    } else {
                        app.ui(ui, region);
                    }
                }
            });
        });
        self.host.set_drag(None);
        if delivered_drop {
            self.drag = None;
        }
        self.used(region, context.globally_used_rect());
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

    fn rect(&self, region: EditorRegion) -> egui::Rect {
        let Some(placement) = self.placement(region) else {
            return egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO);
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
