use std::sync::Arc;

use block::{Block, BlockParent, BlockReferenceList};
use block_client::block_ref::BlockRef;
use block_client::blocks::database::Database;
use block_client::blocks::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
};
use block_client::blocks::database_view::DatabaseView;
use block_client::references::ReferenceResolutionCache;
use block_client::{BlockClient, BlockHandle, ReferenceList};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::block_ui::BlockLabel;
use block_editor_plugin::{egui, EditorHost, EditorRegion};
use uuid::Uuid;

const INTRINSIC_WIDTH: f32 = 400.0;
const ROW_HEIGHT: f32 = 24.0;
const CHROME_HEIGHT: f32 = 90.0;

#[derive(Default)]
pub struct DatabaseApp {
    host: Option<EditorHost>,
    client: Option<Arc<BlockClient>>,
    creation: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<Database>>,
    views: Option<ReferenceList>,
    schema: Option<Uuid>,
    reference_cache: ReferenceResolutionCache,
}

impl DatabaseApp {
    fn views(&self) -> Vec<block::BlockReference> {
        self.views
            .as_ref()
            .map(ReferenceList::read)
            .unwrap_or_default()
            .into_iter()
            .filter(|reference| reference.block_type == DatabaseView::TYPE_ID)
            .collect()
    }

    fn schema(&mut self) -> Option<Uuid> {
        self.reference_cache.poll();
        let client = self.client.clone()?;
        let block = self.block.as_ref()?;
        let schema_reference = block.read()?.schema_id();
        self.schema = self
            .reference_cache
            .resolve(&client, block.id(), schema_reference);
        self.schema
    }
}

impl block_editor_plugin::App for DatabaseApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.views = Some(client.watch_references(BlockReferenceList::Backrefs(block_id)));
        self.block = Some(client.get_block(block_id));
        self.client = Some(client);
        self.host = Some(host);
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
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
        Ok(database.id())
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        Some(egui::vec2(
            INTRINSIC_WIDTH,
            CHROME_HEIGHT + ROW_HEIGHT * self.views().len() as f32,
        ))
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let (Some(host), Some(schema)) = (self.host.clone(), self.schema()) else {
            ui.spinner();
            return;
        };
        host.show_region(EditorRegion::RightSidebar, true);
        host.child(ui, schema, DatabaseSchema::TYPE_ID)
            .keep_active();
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let (Some(host), Some(client), Some(block)) =
            (self.host.clone(), self.client.clone(), self.block.clone())
        else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let views = self.views();
        let types = host.block_types();

        ui.heading("Views");
        if views.is_empty() {
            ui.weak(
                match self.views.as_ref().is_some_and(ReferenceList::is_loaded) {
                    true => "This database has no views yet.",
                    false => "Loading…",
                },
            );
        }
        for view in &views {
            let label = BlockLabel::for_reference(types.as_ref(), view);
            if ui.link(label.rich_text()).clicked() {
                host.open_block(view.id, DatabaseView::TYPE_ID);
            }
        }

        ui.add_space(8.0);
        if ui
            .add_enabled(host.editable(), egui::Button::new("New view"))
            .test_id("database.new-view")
            .clicked()
        {
            client.create_block(DatabaseView::new(BlockRef::Direct(block.id())));
        }
    }
}
