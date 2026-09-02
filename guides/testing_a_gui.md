GUI tests run headless: no window, no GPU, no server. A test builds an editor, drives it the
way a person would, and checks two things — what the block became, and what the editor
painted. They are fast enough to belong in ./scripts/verify: the handful that exist run in
well under a second.

1. What a GUI test may look at

- The block. An editor's job is to turn gestures into operations, so the assertion is on
  the block the operations reached, exactly as in block-e2e.
- The painting, as a snapshot (see 4) - one frame, or a recording of several. This catches
  what an assertion on the block cannot: a control that vanished, a panel that lost its
  contents, a colour that changed.
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

run() paints until the editor asks for no more immediate repaints, so an editor that is
animating — a recording playing, a spinner turning — never lets it return. Drive those a
frame at a time with step(), which paints once however much the editor wanted.

An editor whose manifest claims pan_and_zoom draws into a view the host owns, so its test
is built with EditorTest::viewport(app, host) — the host it was connected to — instead of
EditorTest::new(app). The harness then does what the host does around the main region: it
holds a zoom and an offset, hands the editor host.view() over its region, and answers the
pan, zoom and fit the editor asks for, fitting the content until the first of them arrives.
An editor given EditorTest::new is told nothing about a view and fills its region, which is
what an editor without that capability does anyway.

4. Snapshots of the painting

editor.snapshot(name) writes everything the editor painted into
snapshots/<crate>.<name>.paint, one folder at the root of the repository holding every
painting the workspace accepted: the triangles egui tessellated, and the scrap of texture
each one samples, compressed. It is a few kilobytes rather than the hundreds a screenshot
costs, and it is compared exactly, so nothing about it is flaky.

A painting is a recording: one frame by default, or the frames the test kept. record()
keeps the frame the editor has just painted, and snapshot(name) writes the frames kept
since the last one — or the frame the test is on, if it kept none — so recording a gesture
at a time paints how the editor got somewhere rather than only where it ended up.

    editor.record();
    editor.find("checklist.add").click();
    editor.run();
    editor.record();
    editor.snapshot("adding_an_item_puts_it_on_the_list");

The frames of a recording share one table of textures, so a frame that draws the same text
as the last costs the triangles that draw it and nothing more. Keep recordings to the
frames that say something: every frame is compared, so a frame nobody looks at is one more
way for the test to fail.

- ./scripts/verify accepts whatever the tests paint: it runs them with UPDATE_SNAPSHOTS=1, so
  a new or changed painting is written into snapshots/ rather than failing the run. Commit
  those files with the change that caused them. CI runs ./scripts/verify --check, which sets
  nothing, so a painting that was never committed fails there.
- A changed painting is for a person to review, not for you. They review it in a Paint
  review block, which reads the folder from the repository's dev branch, so a painting is
  reviewed once it has been pushed rather than from the machine that made it. Approving is
  not git and a reviewer is not the tests: a painting nobody approved is new again the next
  time the block is opened, and one nobody had approved before it vanished is not reported
  at all.
- Regenerating them is cheap and mechanical - an egui upgrade rewrites every one - so a
  changed painting is not by itself a failure to explain, and there is nothing in it for you
  to look at. Say in your handoff which paintings changed and why, and leave the images
  alone.
- The exception is a painting you cannot account for: if you do not know why one changed,
  restore the committed file and run the test without UPDATE_SNAPSHOTS - git restore
  snapshots/ && cargo nextest run --workspace - and the failure says which frame changed and
  what moved in it, which is what you needed rather than the image.

A snapshot never holds the font atlas. Each triangle carries the piece of texture it
samples, cut out of the atlas and keyed by what is in it, so where a glyph happened to land
in the atlas cannot reach the file: text an earlier frame drew - a temporary directory's
name, a uuid, the time - repacks the atlas without moving anything in the snapshot. Text
that varies in the frame the test captures is of course a different painting, and still
has to be kept out of it.

Nor does a snapshot hold what a paint callback draws - a plugin's surface, a 3D scene -
since those contents never reach the painter: the snapshot keeps the region and nothing
inside it, so a test of one asserts on the block instead.

An editor that rasterizes its own glyphs rather than egui's - the text editor, which shapes
and rasterizes through HarfBuzz and FreeType - draws from whatever fonts the machine
running the test happens to have, so its content is not a painting to compare. Test what
that editor does to its block, and keep a snapshot for the parts it draws with egui's own
fonts.

5. Running them

Build the workspace, not the package: cargo nextest run --workspace. An editor crate on its
own fails to build, because it is block-app that turns on the windowing features eframe
needs, and cargo only unifies those across a whole-workspace build.
