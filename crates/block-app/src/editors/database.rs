use block::{Block, BlockParent, BlockReferenceList};
use block_client::{
    block_ref::BlockRef,
    blocks::{
        database::Database,
        database_schema::{
            DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
        },
        database_view::DatabaseView,
    },
    BlockClient, BlockHandle, ReferenceList,
};
use eframe::egui;
use egui_material_icons::{icons::ICON_DATABASE, MaterialIcon};
use uuid::Uuid;

use super::{
    database_schema, BlockEditor, BlockLabel, CreatableEditor, DirectEditorCapabilities,
    DirectEditorViewport, EditorAccess, EditorAction, EditorKind,
};
use block_client::references::ReferenceResolutionCache;

const DIRECT_EDITOR_WIDTH: f32 = 400.0;
const DIRECT_EDITOR_ROW_HEIGHT: f32 = 24.0;
const DIRECT_EDITOR_CHROME_HEIGHT: f32 = 90.0;

pub(super) struct DatabaseEditor {
    block: BlockHandle<Database>,
    views: ReferenceList,
    schema: Option<BlockHandle<DatabaseSchema>>,
    reference_cache: ReferenceResolutionCache,
}

impl EditorKind for DatabaseEditor {
    type Block = Database;

    const DISPLAY_NAME: &'static str = "Database";
    const ICON: MaterialIcon = ICON_DATABASE;

    fn open(client: &BlockClient, block: BlockHandle<Database>) -> Self {
        Self::new(client, block)
    }
}

impl CreatableEditor for DatabaseEditor {
    fn create(client: &BlockClient) -> Self {
        let schema = client.create_block(DatabaseSchema::new());
        schema.operate(DatabaseSchemaOperation::AddField {
            field: DatabaseField {
                id: Uuid::new_v4(),
                name: "Name".into(),
                field_type: DatabaseFieldType::String,
                options: Vec::new(),
            },
        });
        let database = client.create_block(Database::new(BlockRef::Direct(schema.id())));
        schema.set_parent(BlockParent::Uuid(database.id()));
        Self::new(client, database)
    }
}

impl DatabaseEditor {
    fn new(client: &BlockClient, block: BlockHandle<Database>) -> Self {
        let views = client.watch_references(BlockReferenceList::Backrefs(block.id()));
        Self {
            block,
            views,
            schema: None,
            reference_cache: ReferenceResolutionCache::default(),
        }
    }

    fn views(&self) -> Vec<block::BlockReference> {
        self.views
            .read()
            .into_iter()
            .filter(|reference| reference.block_type == DatabaseView::TYPE_ID)
            .collect()
    }

    fn ensure_schema(
        &mut self,
        editors: &mut EditorAccess<'_>,
    ) -> Option<&BlockHandle<DatabaseSchema>> {
        self.reference_cache.poll();
        let schema_reference = self.block.read()?.schema_id();
        let referencing_id = self.block.id();
        let client = editors.client_handle();
        let schema_id = self
            .reference_cache
            .resolve(&client, referencing_id, schema_reference)?;
        if self
            .schema
            .as_ref()
            .is_none_or(|schema| schema.id() != schema_id)
        {
            self.schema = Some(editors.client().get_block::<DatabaseSchema>(schema_id));
        }
        self.schema.as_ref()
    }
}

impl BlockEditor for DatabaseEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
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
        Some(egui::vec2(
            DIRECT_EDITOR_WIDTH,
            DIRECT_EDITOR_CHROME_HEIGHT + DIRECT_EDITOR_ROW_HEIGHT * self.views().len() as f32,
        ))
    }

    fn direct_editor_has_right_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let schema = self.ensure_schema(editors)?;
        egui::CollapsingHeader::new("Fields")
            .default_open(true)
            .show(ui, |ui| {
                database_schema::fields_ui(ui, schema);
            });
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let views = self.views();
        let mut action = None;

        ui.heading("Views");
        if views.is_empty() {
            ui.weak(if self.views.is_loaded() {
                "This database has no views yet."
            } else {
                "Loading…"
            });
        }
        for view in &views {
            let label = BlockLabel::for_reference(editors.registry(), view);
            if ui.link(label.rich_text()).clicked() {
                action = Some(EditorAction::OpenBlock {
                    id: view.id,
                    block_type: DatabaseView::TYPE_ID,
                });
            }
        }

        ui.add_space(8.0);
        if ui.button("New view").clicked() {
            editors
                .client()
                .create_block(DatabaseView::new(BlockRef::Direct(self.block.id())));
        }

        action
    }
}
