use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use block_client::blocks::paint_review::PaintReview;
use block_client::blocks::paint_snapshot::PaintSnapshot;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use paint_snapshot::Snapshot;
use uuid::Uuid;

use crate::app::{PaintReviewApp, Status};

mod a_painting_that_changed_on_disk_is_modified;
mod a_painting_that_vanished_is_removed;
mod a_painting_that_was_never_approved_is_new;

const PATH: &str = "snapshots/a_button_is_drawn.paint";

struct Review {
    directory: PathBuf,
    client: Arc<BlockClient>,
    block: BlockHandle<PaintReview>,
}

impl Review {
    fn open() -> (Self, EditorTest<'static, PaintReviewApp>) {
        let directory = std::env::temp_dir().join(format!("paint-review-test-{}", Uuid::new_v4()));
        let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
        let block = client.create_block(PaintReview::new());
        let host = EditorHost::default();
        host.set_editable(true);
        let mut app = PaintReviewApp::default();
        app.connect(host, Arc::clone(&client), block.id());
        app.review_in(directory.clone());
        let review = Self {
            directory,
            client,
            block,
        };
        review.write(PATH, &painting(30));
        let mut editor = EditorTest::new(app);
        editor.run();
        (review, editor)
    }

    fn write(&self, path: &str, painting: &Snapshot) {
        let file = self.directory.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, painting.encode().unwrap()).unwrap();
    }

    fn remove(&self, path: &str) {
        std::fs::remove_file(self.directory.join(path)).unwrap();
    }

    fn approved(&self, path: &str) -> Option<PaintSnapshot> {
        let approval = self.block.read()?.approval(path).cloned()?;
        let id = approval.snapshot.as_direct()?;
        let block = self.client.get_block::<PaintSnapshot>(id);
        let snapshot = block.read()?.to_owned();
        assert_eq!(approval.hash, snapshot.hash());
        assert_eq!(approval.path, snapshot.path());
        Some(snapshot)
    }

    fn reference(&self, path: &str) -> Option<Uuid> {
        self.block.read()?.approval(path)?.snapshot.as_direct()
    }

    fn approvals(&self) -> usize {
        self.block.read().unwrap().approved().len()
    }
}

impl Drop for Review {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).ok();
    }
}

fn status(editor: &mut EditorTest<'_, PaintReviewApp>, path: &str) -> Option<Status> {
    editor
        .app()
        .entries()?
        .into_iter()
        .find(|entry| entry.path == path)
        .map(|entry| entry.status)
}

fn painting(background: u8) -> Snapshot {
    Snapshot {
        size: [24, 16],
        pixels_per_point: 1.0,
        background: [background, background, background, 255],
        primitives: Vec::new(),
        textures: BTreeMap::new(),
    }
}

fn entry_id(path: &str) -> String {
    format!("paint_review.entry.{path}")
}
