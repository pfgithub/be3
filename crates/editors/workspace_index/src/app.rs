use std::{cmp::Ordering, collections::HashMap, sync::Arc};

use block::{BlockReference, BlockReferenceList};
use block_client::{
    block_ref::BlockRef,
    blocks::workspace_index::{WorkspaceIndex, WorkspaceIndexOperation},
    references::{ReferenceClassificationQueue, ReferenceResolutionCache},
    BlockClient, BlockHandle, ReferenceList,
};
use block_editor_plugin::{
    block_ui::{paint_name, BlockLabel, BlockTypes},
    egui,
    egui_material_icons::icons::{
        ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_FOLDER, ICON_GRID_VIEW, ICON_VIEW_LIST,
    },
    EditorHost,
};
use uuid::Uuid;

const INTRINSIC_WIDTH: f32 = 400.0;
const ROW_HEIGHT: f32 = 24.0;
const LARGE_TILE_SIZE: egui::Vec2 = egui::vec2(160.0, 132.0);
const SMALL_TILE_SIZE: egui::Vec2 = egui::vec2(104.0, 88.0);

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum FolderView {
    #[default]
    LargeGrid,
    SmallGrid,
    List,
}

impl FolderView {
    fn label(self) -> &'static str {
        match self {
            Self::LargeGrid => "Large grid",
            Self::SmallGrid => "Small grid",
            Self::List => "List",
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum FolderSort {
    #[default]
    Intrinsic,
    Name,
    Type,
}

impl FolderSort {
    fn label(self) -> &'static str {
        match self {
            Self::Intrinsic => "Intrinsic",
            Self::Name => "Name",
            Self::Type => "Type",
        }
    }
}

struct BrowserEntry {
    entry: BlockRef,
    id: Option<Uuid>,
    intrinsic_index: usize,
    reference: Option<BlockReference>,
}

struct Folder {
    host: EditorHost,
    client: Arc<BlockClient>,
    block: BlockHandle<WorkspaceIndex>,
    references: ReferenceList,
}

#[derive(Default)]
pub struct WorkspaceIndexApp {
    folder: Option<Folder>,
    view: FolderView,
    sort: FolderSort,
    descending: bool,
    selected: Option<BlockRef>,
    reference_cache: ReferenceResolutionCache,
    pending_adds: ReferenceClassificationQueue<()>,
}

impl WorkspaceIndexApp {
    fn poll(&mut self) {
        self.reference_cache.poll();
        let Some(folder) = &self.folder else {
            return;
        };
        for (reference, ()) in self.pending_adds.poll() {
            folder
                .block
                .operate(WorkspaceIndexOperation::Add(reference));
        }
    }

    fn browser_entries(&mut self, types: &dyn BlockTypes) -> Option<Vec<BrowserEntry>> {
        let sort = self.sort;
        let descending = self.descending;
        let folder = self.folder.as_ref()?;
        let index = folder.block.read()?;
        let metadata: HashMap<_, _> = folder
            .references
            .read()
            .into_iter()
            .map(|reference| (reference.id, reference))
            .collect();
        let referencing_id = folder.block.id();
        let client = Arc::clone(&folder.client);
        let cache = &mut self.reference_cache;
        let mut entries: Vec<_> = index
            .entries()
            .iter()
            .enumerate()
            .map(|(intrinsic_index, entry)| {
                let id = cache.resolve(&client, referencing_id, *entry);
                BrowserEntry {
                    entry: *entry,
                    id,
                    intrinsic_index,
                    reference: id.and_then(|id| metadata.get(&id).cloned()),
                }
            })
            .collect();
        drop(index);

        if sort != FolderSort::Intrinsic {
            entries.sort_by(|left, right| {
                let loaded = right.reference.is_some().cmp(&left.reference.is_some());
                if loaded != Ordering::Equal {
                    return loaded;
                }
                let ordering = match sort {
                    FolderSort::Intrinsic => Ordering::Equal,
                    FolderSort::Name => compare_name(left, right, types),
                    FolderSort::Type => compare_type(left, right, types)
                        .then_with(|| compare_name(left, right, types)),
                };
                let ordering = if descending {
                    ordering.reverse()
                } else {
                    ordering
                };
                ordering.then_with(|| left.intrinsic_index.cmp(&right.intrinsic_index))
            });
        }
        Some(entries)
    }

    fn show_grid(
        &mut self,
        ui: &mut egui::Ui,
        types: &dyn BlockTypes,
        entries: &[BrowserEntry],
        tile_size: egui::Vec2,
    ) {
        ui.horizontal_wrapped(|ui| {
            for entry in entries {
                let response = grid_tile(
                    ui,
                    types,
                    entry,
                    tile_size,
                    self.selected == Some(entry.entry),
                );
                if response.clicked() {
                    self.selected = Some(entry.entry);
                }
                if response.double_clicked() {
                    self.open_entry(entry);
                }
            }
        });
    }

    fn show_list(&mut self, ui: &mut egui::Ui, types: &dyn BlockTypes, entries: &[BrowserEntry]) {
        for entry in entries {
            let (label, block_type) = entry.reference.as_ref().map_or_else(
                || {
                    let placeholder = entry
                        .id
                        .map_or_else(|| "broken link".to_owned(), |id| id.to_string());
                    (
                        format!("Loading…  {placeholder}").into(),
                        "Loading…".to_owned(),
                    )
                },
                |reference| {
                    (
                        BlockLabel::for_reference(types, reference).widget_text(ui.style()),
                        type_name(types, reference.block_type),
                    )
                },
            );
            let response = ui.add_sized(
                [ui.available_width(), ROW_HEIGHT],
                egui::Button::selectable(self.selected == Some(entry.entry), label)
                    .right_text(block_type)
                    .truncate(),
            );
            if response.clicked() {
                self.selected = Some(entry.entry);
            }
            if response.double_clicked() {
                self.open_entry(entry);
            }
        }
    }

    fn open_entry(&self, entry: &BrowserEntry) {
        let (Some(folder), Some(reference)) = (&self.folder, entry.reference.as_ref()) else {
            return;
        };
        folder.host.open_block(reference.id, reference.block_type);
    }

    fn handle_drop(&mut self, ui: &mut egui::Ui, rect: egui::Rect, entries: &[BrowserEntry]) {
        let Some(folder) = &self.folder else {
            return;
        };
        let Some(drag) = folder.host.drag() else {
            return;
        };
        if !rect.contains(drag.position) {
            return;
        }
        let valid = drag.block_id != folder.block.id()
            && !entries.iter().any(|entry| entry.id == Some(drag.block_id));
        if drag.dropped {
            if valid {
                let client = Arc::clone(&folder.client);
                let referencing_id = folder.block.id();
                self.pending_adds
                    .push(&client, referencing_id, drag.block_id, ());
            }
            return;
        }
        folder.host.accept_drag(valid);
        let color = if valid {
            ui.visuals().selection.stroke.color
        } else {
            ui.visuals().error_fg_color
        };
        ui.painter().rect_stroke(
            rect.shrink(2.0),
            4.0,
            egui::Stroke::new(2.0_f32, color),
            egui::StrokeKind::Inside,
        );
    }
}

impl block_editor_plugin::App for WorkspaceIndexApp {
    fn connect(&mut self, host: EditorHost, client: BlockClient, block_id: Uuid) {
        let block = client.get_block(block_id);
        let references = client.watch_references(BlockReferenceList::References(block_id));
        self.folder = Some(Folder {
            host,
            client: Arc::new(client),
            block,
            references,
        });
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll();
        let Some(types) = self.folder.as_ref().map(|folder| folder.host.block_types()) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let Some(entries) = self.browser_entries(types.as_ref()) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };

        let drop_rect = ui.available_rect_before_wrap();
        if entries.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.weak("This folder is empty.");
            });
        } else {
            match self.view {
                FolderView::LargeGrid => {
                    self.show_grid(ui, types.as_ref(), &entries, LARGE_TILE_SIZE);
                }
                FolderView::SmallGrid => {
                    self.show_grid(ui, types.as_ref(), &entries, SMALL_TILE_SIZE);
                }
                FolderView::List => self.show_list(ui, types.as_ref(), &entries),
            }
        }
        self.handle_drop(ui, drop_rect, &entries);
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let index = self.folder.as_ref()?.block.read()?;
        let entry_count = index.entries().len().max(1);
        let height = match self.view {
            FolderView::LargeGrid => LARGE_TILE_SIZE.y * entry_count.div_ceil(3) as f32,
            FolderView::SmallGrid => SMALL_TILE_SIZE.y * entry_count.div_ceil(5) as f32,
            FolderView::List => ROW_HEIGHT * entry_count as f32,
        };
        Some(egui::vec2(INTRINSIC_WIDTH, height))
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("folder-view")
                .selected_text(self.view.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.view,
                        FolderView::LargeGrid,
                        format!("{} Large grid", ICON_GRID_VIEW.codepoint),
                    );
                    ui.selectable_value(
                        &mut self.view,
                        FolderView::SmallGrid,
                        format!("{} Small grid", ICON_GRID_VIEW.codepoint),
                    );
                    ui.selectable_value(
                        &mut self.view,
                        FolderView::List,
                        format!("{} List", ICON_VIEW_LIST.codepoint),
                    );
                });
            egui::ComboBox::from_id_salt("folder-sort")
                .selected_text(format!("Sort: {}", self.sort.label()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.sort, FolderSort::Intrinsic, "Intrinsic");
                    ui.selectable_value(&mut self.sort, FolderSort::Name, "Name");
                    ui.selectable_value(&mut self.sort, FolderSort::Type, "Type");
                });
            let direction = if self.descending {
                ICON_ARROW_DOWNWARD
            } else {
                ICON_ARROW_UPWARD
            };
            if ui
                .add_enabled(
                    self.sort != FolderSort::Intrinsic,
                    egui::Button::new(direction),
                )
                .on_hover_text(if self.descending {
                    "Descending"
                } else {
                    "Ascending"
                })
                .clicked()
            {
                self.descending = !self.descending;
            }
        });
    }
}

fn type_name(types: &dyn BlockTypes, block_type: Uuid) -> String {
    types
        .display_name(block_type)
        .map_or_else(|| block_type.to_string(), str::to_owned)
}

fn compare_name(left: &BrowserEntry, right: &BrowserEntry, types: &dyn BlockTypes) -> Ordering {
    let name = |entry: &BrowserEntry| {
        entry.reference.as_ref().map(|reference| {
            BlockLabel::for_reference(types, reference)
                .name
                .to_lowercase()
        })
    };
    name(left).cmp(&name(right))
}

fn compare_type(left: &BrowserEntry, right: &BrowserEntry, types: &dyn BlockTypes) -> Ordering {
    let name = |entry: &BrowserEntry| {
        entry
            .reference
            .as_ref()
            .map(|reference| type_name(types, reference.block_type).to_lowercase())
    };
    name(left).cmp(&name(right))
}

fn grid_tile(
    ui: &mut egui::Ui,
    types: &dyn BlockTypes,
    entry: &BrowserEntry,
    size: egui::Vec2,
    selected: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);
    ui.painter().rect(
        rect,
        5.0,
        visuals.bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );

    let icon_size = if size == LARGE_TILE_SIZE { 52.0 } else { 34.0 };
    let name_size = if size == LARGE_TILE_SIZE { 15.0 } else { 13.0 };
    let (icon, name, automatic, block_type) = entry.reference.as_ref().map_or_else(
        || {
            (
                ICON_FOLDER,
                "Loading…".to_owned(),
                false,
                entry
                    .id
                    .map_or_else(|| "broken link".to_owned(), |id| id.to_string()),
            )
        },
        |reference| {
            let label = BlockLabel::for_reference(types, reference);
            (
                label.icon.unwrap_or(ICON_FOLDER),
                label.name,
                label.automatic,
                types
                    .display_name(reference.block_type)
                    .unwrap_or("Unknown")
                    .to_owned(),
            )
        },
    );
    let center = rect.center();
    ui.painter().text(
        egui::pos2(center.x, rect.top() + 12.0),
        egui::Align2::CENTER_TOP,
        icon.codepoint,
        egui::FontId::new(icon_size, icon.font_family()),
        visuals.text_color(),
    );
    paint_name(
        ui.painter(),
        egui::pos2(center.x, rect.bottom() - 34.0),
        egui::Align2::CENTER_BOTTOM,
        &name,
        egui::FontId::proportional(name_size),
        visuals.text_color(),
        automatic,
    );
    ui.painter().text(
        egui::pos2(center.x, rect.bottom() - 10.0),
        egui::Align2::CENTER_BOTTOM,
        block_type,
        egui::FontId::proportional(11.0),
        ui.visuals().weak_text_color(),
    );
    response
}
