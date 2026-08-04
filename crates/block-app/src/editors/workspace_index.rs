use std::{cmp::Ordering, collections::HashMap};

use block::{Block, BlockReference, BlockReferenceList};
use block_client::{
    blocks::workspace_index::{BlockEntry, WorkspaceIndex, WorkspaceIndexOperation},
    BlockClient, BlockHandle, ReferenceList,
};
use eframe::egui;
use egui_material_icons::icons::{
    ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_FOLDER, ICON_GRID_VIEW, ICON_VIEW_LIST,
};
use uuid::Uuid;

use super::{
    BlockEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess, EditorAction,
    EditorRegistration, SidebarDragPayload,
};

const DIRECT_EDITOR_WIDTH: f32 = 400.0;
const DIRECT_EDITOR_ROW_HEIGHT: f32 = 24.0;
const LARGE_TILE_SIZE: egui::Vec2 = egui::vec2(160.0, 132.0);
const SMALL_TILE_SIZE: egui::Vec2 = egui::vec2(104.0, 88.0);

#[derive(Clone, Copy, PartialEq, Eq)]
enum FolderView {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum FolderSort {
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
    id: Uuid,
    intrinsic_index: usize,
    reference: Option<BlockReference>,
}

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: WorkspaceIndex::TYPE_ID,
        display_name: "Folder",
        icon: ICON_FOLDER,
        create: Some(|client| {
            Box::new(WorkspaceIndexEditor::new(
                client,
                client.create_block(WorkspaceIndex::default()),
            ))
        }),
        open: |client, id| {
            Box::new(WorkspaceIndexEditor::new(
                client,
                client.get_block::<WorkspaceIndex>(id),
            ))
        },
        can_add_child: true,
        can_delete_child: true,
        dynamic_artifact: None,
    }
}

pub(super) struct WorkspaceIndexEditor {
    block: BlockHandle<WorkspaceIndex>,
    references: ReferenceList,
    view: FolderView,
    sort: FolderSort,
    descending: bool,
    selected: Option<Uuid>,
}

impl WorkspaceIndexEditor {
    pub(super) fn new(client: &BlockClient, block: BlockHandle<WorkspaceIndex>) -> Self {
        let references = client.watch_references(BlockReferenceList::References(block.id()));
        Self {
            block,
            references,
            view: FolderView::LargeGrid,
            sort: FolderSort::Intrinsic,
            descending: false,
            selected: None,
        }
    }

    fn browser_entries(&self, editors: &EditorAccess<'_>) -> Option<Vec<BrowserEntry>> {
        let index = self.block.read()?;
        let metadata: HashMap<_, _> = self
            .references
            .read()
            .into_iter()
            .map(|reference| (reference.id, reference))
            .collect();
        let mut entries: Vec<_> = index
            .entries()
            .iter()
            .enumerate()
            .map(|(intrinsic_index, entry)| BrowserEntry {
                id: entry.id,
                intrinsic_index,
                reference: metadata.get(&entry.id).cloned(),
            })
            .collect();
        drop(index);

        if self.sort != FolderSort::Intrinsic {
            entries.sort_by(|left, right| {
                let loaded = right.reference.is_some().cmp(&left.reference.is_some());
                if loaded != Ordering::Equal {
                    return loaded;
                }
                let ordering = match self.sort {
                    FolderSort::Intrinsic => Ordering::Equal,
                    FolderSort::Name => compare_name(left, right),
                    FolderSort::Type => {
                        compare_type(left, right, editors).then_with(|| compare_name(left, right))
                    }
                };
                let ordering = if self.descending {
                    ordering.reverse()
                } else {
                    ordering
                };
                ordering.then_with(|| left.intrinsic_index.cmp(&right.intrinsic_index))
            });
        }
        Some(entries)
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt(("folder-view", self.block.id()))
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
            egui::ComboBox::from_id_salt(("folder-sort", self.block.id()))
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

    fn show_grid(
        &mut self,
        ui: &mut egui::Ui,
        editors: &EditorAccess<'_>,
        entries: &[BrowserEntry],
        tile_size: egui::Vec2,
    ) -> Option<EditorAction> {
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            for entry in entries {
                let response = grid_tile(
                    ui,
                    editors,
                    entry,
                    tile_size,
                    self.selected == Some(entry.id),
                );
                if response.clicked() {
                    self.selected = Some(entry.id);
                }
                if response.double_clicked() {
                    action = open_entry(entry);
                }
            }
        });
        action
    }

    fn show_list(
        &mut self,
        ui: &mut egui::Ui,
        editors: &EditorAccess<'_>,
        entries: &[BrowserEntry],
    ) -> Option<EditorAction> {
        let mut action = None;
        for entry in entries {
            let (label, block_type) = entry.reference.as_ref().map_or_else(
                || (format!("Loading…  {}", entry.id), "Loading…".to_owned()),
                |reference| {
                    (
                        editors
                            .registry()
                            .icon_label(reference.block_type, &reference.name),
                        editors
                            .registry()
                            .display_name(reference.block_type)
                            .map_or_else(|| reference.block_type.to_string(), str::to_owned),
                    )
                },
            );
            let response = ui.add_sized(
                [ui.available_width(), DIRECT_EDITOR_ROW_HEIGHT],
                egui::Button::selectable(self.selected == Some(entry.id), label)
                    .right_text(block_type)
                    .truncate(),
            );
            if response.clicked() {
                self.selected = Some(entry.id);
            }
            if response.double_clicked() {
                action = open_entry(entry);
            }
        }
        action
    }

    fn handle_drop(&self, ui: &mut egui::Ui, rect: egui::Rect, entries: &[BrowserEntry]) {
        let response = ui.interact(
            rect,
            egui::Id::new(("folder-drop-target", self.block.id())),
            egui::Sense::hover(),
        );
        if let Some(dragged) = response.dnd_hover_payload::<SidebarDragPayload>() {
            let valid = self.valid_drop(&dragged, entries);
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
            if valid {
                response.ctx.set_cursor_icon(egui::CursorIcon::Alias);
            }
        }
        if let Some(dragged) = response.dnd_release_payload::<SidebarDragPayload>() {
            if self.valid_drop(&dragged, entries) {
                self.block.operate(WorkspaceIndexOperation::Add(BlockEntry {
                    id: dragged.reference.id,
                }));
            }
        }
    }

    fn valid_drop(&self, dragged: &SidebarDragPayload, entries: &[BrowserEntry]) -> bool {
        dragged.reference.id != self.block.id()
            && !entries.iter().any(|entry| entry.id == dragged.reference.id)
    }
}

fn compare_name(left: &BrowserEntry, right: &BrowserEntry) -> Ordering {
    let left = left
        .reference
        .as_ref()
        .map(|reference| reference.name.to_lowercase());
    let right = right
        .reference
        .as_ref()
        .map(|reference| reference.name.to_lowercase());
    left.cmp(&right)
}

fn compare_type(left: &BrowserEntry, right: &BrowserEntry, editors: &EditorAccess<'_>) -> Ordering {
    let type_name = |entry: &BrowserEntry| {
        entry.reference.as_ref().map(|reference| {
            editors
                .registry()
                .display_name(reference.block_type)
                .map_or_else(|| reference.block_type.to_string(), str::to_owned)
                .to_lowercase()
        })
    };
    type_name(left).cmp(&type_name(right))
}

fn open_entry(entry: &BrowserEntry) -> Option<EditorAction> {
    let reference = entry.reference.as_ref()?;
    Some(EditorAction::OpenBlock {
        id: reference.id,
        block_type: reference.block_type,
    })
}

fn grid_tile(
    ui: &mut egui::Ui,
    editors: &EditorAccess<'_>,
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
    let (icon, name, block_type) = entry.reference.as_ref().map_or_else(
        || (ICON_FOLDER, "Loading…".to_owned(), entry.id.to_string()),
        |reference| {
            (
                editors
                    .registry()
                    .icon(reference.block_type)
                    .unwrap_or(ICON_FOLDER),
                reference.name.clone(),
                editors
                    .registry()
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
    ui.painter().text(
        egui::pos2(center.x, rect.bottom() - 34.0),
        egui::Align2::CENTER_BOTTOM,
        name,
        egui::FontId::proportional(name_size),
        visuals.text_color(),
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

impl BlockEditor for WorkspaceIndexEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn add_child(&self, entry: BlockEntry) -> Option<bool> {
        let index = self.block.read()?;
        let already_present = index
            .entries()
            .iter()
            .any(|existing| existing.id == entry.id);
        drop(index);
        if !already_present {
            self.block.operate(WorkspaceIndexOperation::Add(entry));
        }
        Some(true)
    }

    fn delete_child(&self, entry: BlockEntry) -> Option<bool> {
        let index = self.block.read()?;
        let present = index
            .entries()
            .iter()
            .any(|existing| existing.id == entry.id);
        drop(index);
        if present {
            self.block.operate(WorkspaceIndexOperation::Remove(entry));
        }
        Some(true)
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        let entry_count = self.block.read()?.entries().len().max(1);
        let height = match self.view {
            FolderView::LargeGrid => LARGE_TILE_SIZE.y * entry_count.div_ceil(3) as f32,
            FolderView::SmallGrid => SMALL_TILE_SIZE.y * entry_count.div_ceil(5) as f32,
            FolderView::List => DIRECT_EDITOR_ROW_HEIGHT * entry_count as f32,
        };
        Some(egui::vec2(DIRECT_EDITOR_WIDTH, height))
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.toolbar(ui);
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let Some(entries) = self.browser_entries(editors) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };

        let drop_rect = ui.available_rect_before_wrap();
        let action = if entries.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.weak("This folder is empty.");
            });
            None
        } else {
            match self.view {
                FolderView::LargeGrid => self.show_grid(ui, editors, &entries, LARGE_TILE_SIZE),
                FolderView::SmallGrid => self.show_grid(ui, editors, &entries, SMALL_TILE_SIZE),
                FolderView::List => self.show_list(ui, editors, &entries),
            }
        };
        self.handle_drop(ui, drop_rect, &entries);
        action
    }
}
