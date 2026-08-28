use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use block_client::blocks::paint_review::PaintReview;
use block_client::blocks::paint_snapshot::PaintSnapshot;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use paint_snapshot::Snapshot;
use uuid::Uuid;

use crate::app::{PaintReviewApp, Status};
use crate::download::{Painting, Source};

mod a_painting_that_changed_on_the_branch_is_modified;
mod a_painting_that_vanished_is_removed;
mod a_painting_that_was_never_approved_is_new;
mod a_tree_lists_only_the_paintings_on_it;
mod a_tree_that_says_nothing_useful_is_an_error;

const PATH: &str = "snapshots/a_button_is_drawn.paint";

struct Review {
    branch: Arc<Mutex<Vec<Painting>>>,
    client: Arc<BlockClient>,
    block: BlockHandle<PaintReview>,
}

impl Review {
    fn open() -> (Self, EditorTest<'static, PaintReviewApp>) {
        let branch = Arc::new(Mutex::new(Vec::new()));
        let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
        let block = client.create_block(PaintReview::new());
        let host = EditorHost::default();
        host.set_editable(true);
        let mut app = PaintReviewApp::default();
        app.connect(host, Arc::clone(&client), block.id());
        app.review(Source::Fixed(Arc::clone(&branch)));
        let review = Self {
            branch,
            client,
            block,
        };
        review.write(PATH, &painting(30));
        let mut editor = EditorTest::new(app);
        editor.run();
        (review, editor)
    }

    fn write(&self, path: &str, painting: &Snapshot) {
        let data = painting.encode().unwrap();
        let mut branch = self.branch.lock().unwrap();
        branch.retain(|held| held.path != path);
        branch.push(Painting {
            path: path.to_owned(),
            hash: PaintSnapshot::fingerprint(&data),
            data,
        });
        branch.sort_by(|left, right| left.path.cmp(&right.path));
    }

    fn remove(&self, path: &str) {
        self.branch.lock().unwrap().retain(|held| held.path != path);
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
