use block_plugin_api::{EditorInstanceId, EditorMessage, EditorRegion, Message, ScreenLayout};
use eframe::egui;

use super::{instances::Instances, presenter::PresenterStatus};

pub(super) struct RuntimeCore {
    pub(super) surface: u32,
    pub(super) status: PresenterStatus,
    pub(super) instances: Instances,
    pub(super) layout: ScreenLayout,
    sent: Vec<block_plugin_api::ScreenRequest>,
    pub(super) pass: u64,
}

impl RuntimeCore {
    pub(super) fn new(surface: u32) -> Self {
        Self {
            surface,
            status: PresenterStatus::waiting(),
            instances: Instances::default(),
            layout: ScreenLayout::default(),
            sent: Vec::new(),
            pass: 0,
        }
    }

    pub(super) fn begin_pass(&mut self, pass: u64) -> Option<(Vec<Message>, bool)> {
        if self.pass == pass {
            return None;
        }
        let previous = self.pass;
        self.pass = pass;
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
        Some((messages, awaited))
    }

    pub(super) fn region_size(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<egui::Vec2> {
        self.instances.region_size(instance, region)
    }

    pub(super) fn creation_ready(&self, instance: EditorInstanceId) -> bool {
        self.instances.creation_ready(instance)
    }

    pub(super) fn aspect_ratio(&self, instance: EditorInstanceId) -> Option<f32> {
        self.instances.aspect_ratio(instance)
    }

    pub(super) fn intrinsic_size(&self, instance: EditorInstanceId) -> Option<egui::Vec2> {
        self.instances.intrinsic_size(instance)
    }

    pub(super) fn close(&mut self, instance: EditorInstanceId) -> Option<Vec<Message>> {
        self.instances
            .remove(instance)
            .then(|| vec![Message::Editor(EditorMessage::Close { instance })])
    }
}

pub(super) enum SurfaceSelection {
    Selected(u32),
    Evict(String),
}

pub(super) fn select_surface<'a>(
    plugin_id: &str,
    runtimes: impl Iterator<Item = (&'a String, &'a RuntimeCore)>,
) -> Option<SurfaceSelection> {
    let runtimes: Vec<_> = runtimes.collect();
    if let Some((_, runtime)) = runtimes.iter().find(|(id, _)| id.as_str() == plugin_id) {
        return Some(SurfaceSelection::Selected(runtime.surface));
    }
    if let Some(surface) = (0..super::presenter::MAX_SURFACES).find(|surface| {
        !runtimes
            .iter()
            .any(|(_, runtime)| runtime.surface == *surface)
    }) {
        return Some(SurfaceSelection::Selected(surface));
    }
    runtimes
        .into_iter()
        .filter(|(_, runtime)| runtime.instances.is_empty())
        .min_by_key(|(_, runtime)| runtime.pass)
        .map(|(id, _)| SurfaceSelection::Evict(id.clone()))
}
