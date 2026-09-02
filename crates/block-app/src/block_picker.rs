use std::collections::{BTreeMap, HashSet};

use block::{Block, BlockParent};
use block_client::{
    blocks::infinite_canvas::InfiniteCanvas,
    properties::{self, BlockName},
    BlockClient, CachedBlock,
};
use eframe::egui;
use egui_material_icons::MaterialIcon;
use uuid::Uuid;

use crate::{
    editors::{
        BlockCreation, BlockEditor, BlockLabel, CreationStep, EditorAccess, EditorRegistry,
        PendingCreation,
    },
    slide_templates::SlideTemplate,
};

const ADD_TILE_SIZE: egui::Vec2 = egui::vec2(132.0, 124.0);

#[derive(PartialEq, Eq, Clone, Copy)]
enum BlockPickerTab {
    Add,
    Templates,
    LinkExisting,
}

struct PendingBlock {
    block_type: Uuid,
    creation: Box<dyn PendingCreation>,
    creating: bool,
}

pub struct BlockPickerResult {
    pub id: Uuid,
    pub block_type: Uuid,
    pub author: Uuid,
    pub properties: BTreeMap<Uuid, Vec<u8>>,
    pub linked: bool,
}

pub struct BlockPicker {
    id: Uuid,
    open: bool,
    tab: BlockPickerTab,
    search: String,
    excluded: HashSet<Uuid>,
    allowed: HashSet<Uuid>,
    pending_block: Option<PendingBlock>,
    error: Option<String>,
}

impl Default for BlockPicker {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            open: false,
            tab: BlockPickerTab::Add,
            search: String::new(),
            excluded: HashSet::new(),
            allowed: HashSet::new(),
            pending_block: None,
            error: None,
        }
    }
}

impl BlockPicker {
    pub fn open(&mut self, excluded: impl IntoIterator<Item = Uuid>) {
        self.allowed.clear();
        self.open_on_tab(excluded, BlockPickerTab::Add);
    }

    pub fn open_for_types(
        &mut self,
        excluded: impl IntoIterator<Item = Uuid>,
        allowed: impl IntoIterator<Item = Uuid>,
    ) {
        self.allowed = allowed.into_iter().collect();
        self.open_on_tab(excluded, BlockPickerTab::Add);
    }

    pub fn open_templates_for_types(
        &mut self,
        excluded: impl IntoIterator<Item = Uuid>,
        allowed: impl IntoIterator<Item = Uuid>,
    ) {
        self.allowed = allowed.into_iter().collect();
        self.open_on_tab(excluded, BlockPickerTab::Templates);
    }

    fn open_on_tab(&mut self, excluded: impl IntoIterator<Item = Uuid>, tab: BlockPickerTab) {
        self.open = true;
        self.tab = tab;
        self.search.clear();
        self.excluded = excluded.into_iter().collect();
    }

    pub fn is_open(&self) -> bool {
        self.open || self.pending_block.is_some()
    }

    fn show_modal(
        &mut self,
        context: &egui::Context,
        client: &BlockClient,
        registry: &EditorRegistry,
    ) -> (Option<Uuid>, Option<SlideTemplate>, Option<CachedBlock>) {
        const FOOTER_RESERVE: f32 = 44.0;

        let mut new_type = None;
        let mut template = None;
        let mut linked = None;
        let mut close = false;
        let screen = context.content_rect();
        let modal_width = (screen.width() - 32.0).clamp(280.0, 640.0);
        let modal_height = (screen.height() - 32.0).clamp(320.0, 720.0);
        let stacked_tabs = modal_width < 480.0;
        let response =
            egui::Modal::new(egui::Id::new(("block-picker", self.id))).show(context, |ui| {
                ui.set_width(modal_width);
                ui.set_height(modal_height);
                ui.heading("Add block");
                ui.add_space(8.0);
                if stacked_tabs {
                    ui.horizontal(|ui| show_tabs(ui, &mut self.tab));
                    ui.add_space(4.0);
                    ui.separator();
                }
                let content_height = (ui.available_height() - FOOTER_RESERVE).max(120.0);
                ui.horizontal_top(|ui| {
                    if !stacked_tabs {
                        ui.vertical(|ui| {
                            ui.set_width(140.0);
                            show_tabs(ui, &mut self.tab);
                        });
                        ui.separator();
                    }
                    ui.vertical(|ui| {
                        ui.set_min_width(ui.available_width());
                        match self.tab {
                            BlockPickerTab::Add => {
                                new_type =
                                    show_add_grid(ui, registry, &self.allowed, content_height);
                            }
                            BlockPickerTab::Templates => {
                                template = show_templates_grid(ui, content_height);
                            }
                            BlockPickerTab::LinkExisting => {
                                linked = show_link_content(
                                    ui,
                                    &mut self.search,
                                    &self.excluded,
                                    &self.allowed,
                                    client,
                                    registry,
                                    content_height,
                                );
                            }
                        }
                    });
                });
                ui.add_space(8.0);
                ui.separator();
                egui::Sides::new().show(
                    ui,
                    |_ui| {},
                    |ui| {
                        close = ui.button("Close").clicked();
                    },
                );
            });
        if close
            || new_type.is_some()
            || template.is_some()
            || linked.is_some()
            || response.should_close()
        {
            self.open = false;
        }
        (new_type, template, linked)
    }

    pub fn handle(
        &mut self,
        context: &egui::Context,
        editors: &mut EditorAccess<'_>,
        created_parent: BlockParent,
    ) -> Option<BlockPickerResult> {
        let mut result = self.show_creation_options(context, editors, created_parent);
        if result.is_none() && self.open {
            let (new_type, template, linked) =
                self.show_modal(context, editors.client(), editors.registry());
            if let Some(block_type) = new_type {
                result = self.create_registered_block(editors, block_type, created_parent);
            } else if let Some(template) = template {
                result = Some(Self::finish_template(editors, template, created_parent));
            } else if let Some(block) = linked {
                editors.ensure(block.id, block.block_type);
                result = Some(BlockPickerResult {
                    id: block.id,
                    block_type: block.block_type,
                    author: block.author,
                    properties: block.properties,
                    linked: true,
                });
            }
        }
        self.show_error(context);
        result
    }

    fn create_registered_block(
        &mut self,
        editors: &mut EditorAccess<'_>,
        block_type: Uuid,
        parent: BlockParent,
    ) -> Option<BlockPickerResult> {
        match editors.registry().create(editors.client(), block_type) {
            Some(BlockCreation::Created(editor)) => {
                Some(Self::finish_creation(editors, editor, block_type, parent))
            }

            Some(BlockCreation::Options(creation)) => {
                self.pending_block = Some(PendingBlock {
                    block_type,
                    creation,
                    creating: false,
                });
                None
            }
            None => {
                self.error = Some(format!("Could not create block type {block_type}"));
                None
            }
        }
    }

    fn show_creation_options(
        &mut self,
        context: &egui::Context,
        editors: &mut EditorAccess<'_>,
        parent: BlockParent,
    ) -> Option<BlockPickerResult> {
        let mut pending = self.pending_block.take()?;
        let title = editors
            .registry()
            .display_name(pending.block_type)
            .unwrap_or("block");
        let mut create = false;
        let mut cancel = false;
        let response =
            egui::Modal::new(egui::Id::new(("block-picker-create", self.id))).show(context, |ui| {
                ui.set_min_width(320.0);
                ui.heading(format!("New {title}"));
                ui.add_space(8.0);
                let step = pending.creation.ui(ui, editors);
                ui.separator();
                ui.horizontal(|ui| {
                    match step {
                        CreationStep::Options(ready) => {
                            create = ui
                                .add_enabled(
                                    ready && !pending.creating,
                                    egui::Button::new("Create"),
                                )
                                .on_disabled_hover_text("Fill in the options first")
                                .clicked();
                        }
                        CreationStep::Working => {
                            ui.spinner();
                            ui.label(format!("Creating {title}..."));
                            create = true;
                        }
                    }
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if cancel || response.should_close() {
            return None;
        }
        if !create && !pending.creating {
            self.pending_block = Some(pending);
            return None;
        }
        pending.creating = true;
        match pending.creation.create(editors.client()) {
            Ok(Some(editor)) => Some(Self::finish_creation(
                editors,
                editor,
                pending.block_type,
                parent,
            )),
            Ok(None) => {
                self.pending_block = Some(pending);
                None
            }
            Err(error) => {
                self.error = Some(error);
                None
            }
        }
    }

    fn finish_template(
        editors: &mut EditorAccess<'_>,
        template: SlideTemplate,
        parent: BlockParent,
    ) -> BlockPickerResult {
        let canvas = crate::slide_templates::build_template_canvas(template);
        let block = editors.client().create_block(canvas);
        let id = block.id();
        block.set_parent(parent);
        editors.ensure(id, InfiniteCanvas::TYPE_ID);
        BlockPickerResult {
            id,
            block_type: InfiniteCanvas::TYPE_ID,
            author: editors.client().account_id(),
            properties: BTreeMap::new(),
            linked: false,
        }
    }

    fn finish_creation(
        editors: &mut EditorAccess<'_>,
        editor: Box<dyn BlockEditor>,
        block_type: Uuid,
        parent: BlockParent,
    ) -> BlockPickerResult {
        editor.set_parent(parent);
        let id = editor.id();
        let mut result_properties = BTreeMap::new();
        if let Some(value) = editor.name() {
            result_properties.insert(
                properties::NAME,
                properties::encode_name(&BlockName {
                    manual: false,
                    value,
                }),
            );
        }
        editors.insert(editor);
        BlockPickerResult {
            id,
            block_type,
            author: editors.client().account_id(),
            properties: result_properties,
            linked: false,
        }
    }

    fn show_error(&mut self, context: &egui::Context) {
        let Some(error) = self.error.clone() else {
            return;
        };
        let response =
            egui::Modal::new(egui::Id::new(("block-picker-error", self.id))).show(context, |ui| {
                ui.set_min_width(280.0);
                ui.heading("Block picker error");
                ui.add_space(8.0);
                ui.colored_label(ui.visuals().error_fg_color, error);
                ui.add_space(8.0);
                ui.button("Dismiss").clicked()
            });
        if response.inner || response.should_close() {
            self.error = None;
        }
    }
}

fn show_tabs(ui: &mut egui::Ui, tab: &mut BlockPickerTab) {
    if ui
        .selectable_label(*tab == BlockPickerTab::Add, "Add")
        .clicked()
    {
        *tab = BlockPickerTab::Add;
    }
    if ui
        .selectable_label(*tab == BlockPickerTab::Templates, "Templates")
        .clicked()
    {
        *tab = BlockPickerTab::Templates;
    }
    if ui
        .selectable_label(*tab == BlockPickerTab::LinkExisting, "Link existing")
        .clicked()
    {
        *tab = BlockPickerTab::LinkExisting;
    }
}

fn show_templates_grid(ui: &mut egui::Ui, max_height: f32) -> Option<SlideTemplate> {
    let mut selected = None;
    egui::ScrollArea::vertical()
        .max_height(max_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for template in SlideTemplate::ALL {
                    let label = template.label();
                    let response =
                        show_add_tile(ui, Some(template.icon()), label).on_hover_text(label);
                    if response.clicked() {
                        selected = Some(template);
                    }
                }
            });
        });
    selected
}

fn show_add_grid(
    ui: &mut egui::Ui,
    registry: &EditorRegistry,
    allowed: &HashSet<Uuid>,
    max_height: f32,
) -> Option<Uuid> {
    let mut selected = None;
    egui::ScrollArea::vertical()
        .max_height(max_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut show_section = |ui: &mut egui::Ui, default: bool| {
                ui.horizontal_wrapped(|ui| {
                    for &(label, block_type, is_default) in registry.new_block_actions() {
                        if is_default != default
                            || !(allowed.is_empty() || allowed.contains(&block_type))
                        {
                            continue;
                        }
                        let response = show_add_tile(ui, registry.icon(block_type), label)
                            .on_hover_text(label);
                        if response.clicked() {
                            selected = Some(block_type);
                        }
                    }
                });
            };
            show_section(ui, true);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            show_section(ui, false);
        });
    selected
}

fn show_add_tile(ui: &mut egui::Ui, icon: Option<MaterialIcon>, label: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(ADD_TILE_SIZE, egui::Sense::click());
    let visuals = ui.style().interact(&response);
    let painter = ui.painter();
    painter.rect(
        rect,
        5.0,
        visuals.bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let preview_center = egui::pos2(rect.center().x, rect.top() + (rect.height() - 28.0) / 2.0);
    if let Some(icon) = icon {
        painter.text(
            preview_center,
            egui::Align2::CENTER_CENTER,
            icon.codepoint,
            egui::FontId::new(40.0, icon.font_family()),
            visuals.text_color(),
        );
    }
    let caption = icon.map_or_else(
        || label.to_owned(),
        |icon| format!("{} {label}", icon.codepoint),
    );
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 14.0),
        egui::Align2::CENTER_CENTER,
        caption,
        egui::FontId::proportional(13.0),
        visuals.text_color(),
    );
    response
}

fn show_link_content(
    ui: &mut egui::Ui,
    search: &mut String,
    excluded: &HashSet<Uuid>,
    allowed: &HashSet<Uuid>,
    client: &BlockClient,
    registry: &EditorRegistry,
    max_height: f32,
) -> Option<CachedBlock> {
    ui.add(egui::TextEdit::singleline(search).hint_text("Search by name or UUID"));
    ui.separator();
    let query = search.trim().to_lowercase();
    let blocks: Vec<_> = client
        .cached_blocks()
        .into_iter()
        .filter(|block| !excluded.contains(&block.id))
        .filter(|block| allowed.is_empty() || allowed.contains(&block.block_type))
        .filter(|block| {
            query.is_empty()
                || BlockLabel::for_cached(registry, block)
                    .name
                    .to_lowercase()
                    .contains(&query)
                || block.id.to_string().contains(&query)
        })
        .collect();
    let mut selected = None;
    egui::ScrollArea::vertical()
        .max_height(max_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if blocks.is_empty() {
                ui.weak(if query.is_empty() {
                    "No blocks are available to link."
                } else {
                    "No matching blocks."
                });
            }
            for block in &blocks {
                let label = BlockLabel::for_cached(registry, block).widget_text(ui.style());
                if ui
                    .button(label)
                    .on_hover_text(block.id.to_string())
                    .clicked()
                {
                    selected = Some(block.clone());
                }
            }
        });
    selected
}
