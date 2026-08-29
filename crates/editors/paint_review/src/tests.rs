use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use block::BlockParent;
use block_client::block_ref::BlockRef;
use block_client::blocks::paint_review::{ApprovedPainting, PaintReview, PaintReviewOperation};
use block_client::blocks::paint_snapshot::PaintSnapshot;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{egui, App as _, EditorHost};
use block_ui_test::EditorTest;
use paint_snapshot::{Content, Frame, Primitive, Snapshot, Texture, Triangle, Vertex};
use uuid::Uuid;

use crate::app::{PaintReviewApp, Status};
use crate::download::{Painting, Source};

mod a_painting_is_only_rastered_once;
mod a_painting_that_changed_on_the_branch_is_modified;
mod a_painting_that_vanished_is_removed;
mod a_painting_that_was_never_approved_is_new;
mod a_recording_is_reviewed_one_frame_at_a_time;
mod a_tree_lists_only_the_paintings_on_it;
mod a_tree_that_says_nothing_useful_is_an_error;
mod choosing_a_painting_rasters_the_one_it_was_approved_as;
mod paintings_are_downloaded_a_few_at_a_time;
mod the_difference_counts_the_pixels_that_changed;
mod the_difference_shows_the_pixels_that_changed;
mod the_painting_can_be_zoomed_in_on;
mod unapproving_a_painting_makes_it_new_again;

const PATH: &str = "counter.a_button_is_drawn.paint";

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

    fn approve(&self, path: &str, painting: &Snapshot) {
        let data = painting.encode().unwrap();
        let hash = PaintSnapshot::fingerprint(&data);
        let created = self.client.create_block(PaintSnapshot::new(path, data));
        created.set_parent(BlockParent::Uuid(self.block.id()));
        self.block.operate(PaintReviewOperation::Approve {
            painting: ApprovedPainting {
                path: path.to_owned(),
                hash,
                snapshot: BlockRef::Direct(created.id()),
            },
        });
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

    fn orphaned(&self, id: Uuid) -> bool {
        self.client
            .get_block::<PaintSnapshot>(id)
            .relationships()
            .parent
            == BlockParent::Orphaned
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
    recording(&[background])
}

fn recording(backgrounds: &[u8]) -> Snapshot {
    Snapshot {
        frames: backgrounds
            .iter()
            .map(|background| Frame {
                size: [24, 16],
                pixels_per_point: 1.0,
                background: [*background, *background, *background, 255],
                primitives: Vec::new(),
            })
            .collect(),
        textures: BTreeMap::new(),
    }
}

fn marked(background: u8, left: f32) -> Snapshot {
    let white = Texture::encode([1, 1], &[[255, 255, 255, 255]]).unwrap();
    let corner = |x: f32, y: f32| Vertex {
        pos: [x, y],
        uv: [0.5, 0.5],
        color: [220, 40, 60, 255],
    };
    Snapshot {
        frames: vec![Frame {
            size: [24, 16],
            pixels_per_point: 1.0,
            background: [background, background, background, 255],
            primitives: vec![Primitive {
                clip: [0.0, 0.0, 24.0, 16.0],
                content: Content::Mesh(vec![Triangle {
                    texture: 0,
                    corners: [
                        corner(left, 3.0),
                        corner(left + 7.0, 3.0),
                        corner(left, 12.0),
                    ],
                }]),
            }],
        }],
        textures: BTreeMap::from([(0, white)]),
    }
}

fn entry_id(path: &str) -> String {
    format!("paint_review.entry.{path}")
}
