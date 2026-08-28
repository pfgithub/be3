GUI tests run headless: no window, no GPU, no server. A test builds an editor, drives it the
way a person would, and checks two things — what the block became, and what the editor
painted. They are fast enough to belong in ./scripts/verify: the six that exist run in
well under a second.

1. What a GUI test may look at

- The block. An editor's job is to turn gestures into operations, so the assertion is on
  the block the operations reached, exactly as in block-e2e.
- The painting, as a snapshot (see 4). This catches what an assertion on the block cannot:
  a control that vanished, a panel that lost its contents, a colour that changed.
- Never a coordinate, a widget's size, or the order of the accessibility tree. Those change
  whenever anyone touches a layout and say nothing about whether the editor works.

2. Give the widgets test ids

Widgets are found by an id the test names, never by their label: renaming a button, giving
it an icon, or moving it to a sidebar then leaves the tests alone.

    use block_editor_plugin::block_ui::test_id::TestId;

    if ui.button("Add").test_id("checklist.add").clicked() {

test_id writes an author id onto the widget's accessibility node, which is what
block-ui-test searches for. AccessKit is off unless a screen reader or a test turns it on,
so the call allocates nothing in the app; it does take the context's lock for a moment, so
tag the widgets tests reach for rather than every widget. Name them
`<editor>.<what it does>`, and where there are many of a kind, key them by whatever the
block itself keys them by (`checklist.item.3.done` — the operation takes that index too),
never by the order they happen to be drawn in.

3. Write the test

Tests live inside the editor's crate, one test per file under src/tests/, like every other
test in the repository. block-ui-test's EditorTest lays the editor out the way the host
does — toolbar above, sidebars either side, the editor in the middle — and runs it in a
headless egui.

    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Checklist::default());
    let mut app = ChecklistApp::default();
    app.connect(Default::default(), client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();

    editor.find("checklist.draft").click();
    editor.run();
    editor.find("checklist.draft").type_text("buy milk");
    editor.run();
    editor.find("checklist.add").click();
    editor.run();

    assert_eq!(items(&block), [("buy milk".to_owned(), false)]);
    editor.snapshot("adding_an_item_puts_it_on_the_list");

A block client that was never connected is a whole client: it creates blocks, applies
operations and reads them back locally, so an editor test needs no server. Call run() after
every gesture — egui only sees an event on the frame after it arrives — and click a text
field before typing into it. find() panics with the whole accessibility tree when nothing
matches, which is usually a widget that was never given a test id.

4. Snapshots of the painting

editor.snapshot(name) records everything the editor painted into
<crate>/snapshots/<name>.paint: the triangles egui tessellated, and the scrap of texture
each one samples, compressed. It is a few kilobytes rather than the hundreds a screenshot
costs, and it is compared exactly, so nothing about it is flaky.

- Accept a new or changed painting with UPDATE_SNAPSHOTS=1 cargo nextest run --workspace.
  A test that fails without it says what changed and leaves the accepted file as it was.
- Look at them in a Paint review block, which is what a person reviews them with. It
  downloads every .paint file on the repository's dev branch from GitHub, so a painting is
  reviewed once it has been pushed rather than from the machine that made it, and works the
  same in the browser as on the desktop. It sorts them into the ones it has never seen, the
  ones whose contents changed since they were approved, and the ones that have gone.
  Choosing one renders it, a changed one toggles between the painting that was approved and
  the one on the branch, and approving it keeps a copy of the painting - a block of its own
  - and the hash.
- Approving is not git, and a reviewer is not the tests: a painting nobody approved is new
  again the next time the block is opened, and one nobody had approved before it vanished
  is not reported at all.

A snapshot never holds the font atlas. Each triangle carries the piece of texture it
samples, cut out of the atlas and keyed by what is in it, so where a glyph happened to land
in the atlas cannot reach the file: text an earlier frame drew - a temporary directory's
name, a uuid, the time - repacks the atlas without moving anything in the snapshot. Text
that varies in the frame the test captures is of course a different painting, and still
has to be kept out of it.

Regenerating them is cheap and mechanical - an egui upgrade rewrites every one - so a
changed snapshot is not by itself a failure to explain. You should not look at the image,
images are for a human to review later. What the review block renders is close to what the
app draws rather than identical: it blends in sRGB where the GPU blends in linear light, and a
region drawn by a paint callback (a plugin's surface, a 3D scene) is a magenta outline,
since its contents never reach the painter.

5. Running them

Build the workspace, not the package: cargo nextest run --workspace. An editor crate on its
own fails to build, because it is block-app that turns on the windowing features eframe
needs, and cargo only unifies those across a whole-workspace build.
