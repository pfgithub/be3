# Task: plugin editors own their whole frame

Today a plugin editor is cut into regions — main, toolbar, left sidebar, right sidebar — that the
**host** lays out and draws separately, each in an `egui::Context` of its own. Replace that with:
**one context, one surface, one pass per instance, covering the whole frame, laid out by the
plugin.** A plugin editor draws its own toolbar and sidebars inside the frame it was given.

When a sub-editor is selected — a block being edited inside an infinite canvas or inside a text
block — **the sub-editor takes over the frame**. It gets its own full-size chrome in the frame,
draws its content where its content lives, and leaves everything else transparent so the layer
behind still renders there. That transparency is the hole: the framework, not the plugin, decides
where it is, so a plugin cannot paint over its parent or over the app.

Taking over means **replacing** the parent's chrome, not nesting inside it. The bands keep
exactly the geometry they had; only their contents change. That is the point: the canvas viewport
does not shrink when you select a block, so nothing moves and the zoom does not shift. Nesting a
child's chrome inside the parent's would squeeze the canvas into a smaller viewport and break its
view every time you select or deselect.

This is what keeps the property the region split was protecting: a spreadsheet edited inside a
canvas gets a full-height sidebar at the edge of the frame, not one squeezed into the block's
rect.

Everything under "Findings" was verified by reading the code. Trust it, but re-check anything
that looks stale.

## Findings you don't need to re-derive

- **Host-side frame layout lives in `crates/block-app/src/editors.rs:690`** and is what moves
  into the plugin. It owns: a top `egui::Panel` toolbar, resizable left/right panels (240 default,
  200–340 range), a **compact mode** under `COMPACT_DIRECT_EDITOR_WIDTH` that turns sidebars into
  floating `egui::Window`s, `editor_scope` read-only greying, and the pan/zoom viewport for the
  content. All of that behaviour has to survive.
- `crates/block-app/src/plugin_host/runtime.rs:375` (`editor_ui`) is called once per region;
  `crates/block-app/src/plugin_host/instances.rs:214` (`report`) keys screens by
  `(instance, region)`; `crates/block-app/src/editors/plugin.rs` implements
  `direct_editor_top_bar` / `_left_sidebar` / `_right_sidebar` / `_ui` by forwarding each to a
  region. All of that goes away for plugins.
- `crates/block-editor-plugin/src/panes.rs` keys `Pane` (context + `egui_wgpu::Renderer` + punch
  resources) by `ScreenId`. `crates/block-editor-plugin/src/egui_session.rs` (`EguiSession`) is
  already per instance and already holds every region's state; its `run()` takes a region.
  `crates/block-editor-plugin/src/screens.rs` owns the sessions and calls `ScreenLayout::stacked`.
- **The plugin owns the surface packing.** `ScreenLayout::stacked`
  (`crates/block-plugin-api/src/lib.rs:72`) runs plugin-side and is published as `Message::Layout`
  (`block-editor-plugin/src/runtime.rs`, `replace_surface`); the host samples the rects it is
  told. Repacking needs no negotiation.
- **All regions of an instance already share one scale factor** — the host passes its single
  `ui.ctx().pixels_per_point()` (`plugin_host/runtime.rs:419`).
- **Rotation only exists on the preview path.** The editor path is always axis-aligned
  (`Quad::upright`, `plugin_host/runtime.rs:415`); arbitrary corners only in `PluginEditor::render`
  (`editors/plugin.rs`, `context.corners`).
- The infinite canvas is a **host** editor, not a plugin, and already forwards the tab's chrome to
  its focused child: `crates/block-app/src/editors/infinite_canvas/block_editor.rs:142`, `:199`,
  `:222`. It must become a frame owner under the new model, using the same shared layout.
- The presentation editor is the case that constrains everything.
  `crates/editors/presentation/src/app.rs` places the *same* slide block four ways in one frame:
  every slide as a `ChildMode::Preview` child in the filmstrip (`filmstrip_tile`, :452); the
  selected slide as a `keep_active` child on the stage with the slide's own left sidebar carved
  240px out of the stage (`slide_sidebar`, :210); the slide's `child_part(Toolbar)` inline in its
  toolbar row (:639); and a right sidebar that is nothing but the slide's
  `child_part(RightSidebar)` (:658).
- egui is 0.34.3 from crates.io (not vendored). **egui prunes non-ROOT viewports that were not
  "used" when their parent runs** (`egui-0.34.3/src/context.rs:2673`); a viewport parented to ROOT
  survives as long as ROOT is never run. Give every viewport a stable non-ROOT `ViewportId` and
  never run `ViewportId::ROOT`.
- `floating_rects` in `egui_session.rs` already collects layers at `Order >= Middle`.
- Per-region contexts force editors to duplicate uploads: `crates/editors/image_block/src/app.rs`
  (`self.panes`, decoding the same image once per region) and
  `crates/editors/pixel_art/src/canvas.rs` (`Pane` with its own `TextureHandle`). Also check
  `map`, `pdf`, `paint_review`, `pixel_ray_tracer`.

## The model

**Frame owners.** A tab has a stack of frame owners. At the bottom, the tab's own editor. On top,
each successively selected sub-editor. Each frame owner is handed a **frame rect** and renders
**one surface** for it, in **one egui pass**, in **one context**.

**Chrome is laid out by a shared module, not by the plugin and not by the host.** Put it in
`crates/block-ui` so host editors and plugins produce identical frames. Given a frame rect it
yields the bands — toolbar row, left sidebar, right sidebar, content — and carries the behaviour
that lives in `editors.rs:690` today: resizable sidebars, compact mode, read-only greying. The
band geometry is a function of the frame alone, never of how many owners are in the stack.

**Each band belongs to exactly one owner per frame, and handover is always a replacement.** When
a child is selected the child owns every chrome band; the parent is told its chrome is hidden this
frame and skips drawing it, keeping only its content band, unchanged. Nothing in the frame moves.
There is no nesting mode: a child's chrome is never placed inside the parent's.

Plugins keep writing `toolbar_ui`, `left_sidebar_ui`, `right_sidebar_ui`, `ui`; the plugin-side
framework calls them into whichever bands that instance owns this frame, and skips the ones it
does not. A plugin cannot address the frame directly and cannot tell whether it is the outermost
owner or a child that took over.

On handover the toolbar is entirely the child's, so **the framework — not the plugin — draws a
breadcrumb and an exit at a fixed spot in the toolbar band** ("Canvas › Spreadsheet ✕", Escape to
leave). A plugin must not be able to drop the only way back out.

**Content.** For the tab's own editor, content is the content band. For a sub-editor, content is
the rect its parent placed it at — the block's rect on the canvas, the run of text it sits in —
clipped to what is visible. The parent reports that rect through the existing child placement
mechanism; the framework hands it to the child as its content rect. A replaced parent's content
band is bit-for-bit the rect it had before the selection.

**Holes.** A frame surface is transparent wherever its owner does not paint: outside its bands,
and inside the content band outside its content rect. The layer beneath shows through there, so
the canvas keeps drawing its background, its other blocks and its previews around the block being
edited. Nothing else is needed to make a hole — the surface is already cleared transparent. The
existing `punch` shader stays for holes *inside* painted areas (embedded children).

**Compositing and z.** The host blits, bottom-up through the stack, each frame's painted area, and
blits each frame's **floating rects on a second pass above the frames beneath it**, as
rect-disjoint pieces (base pieces = painted area minus floating rects) so nothing is blended
twice. That is what lets the deck's "Add slide" dropdown fall over the stage while the slide owns
the frame above it. Floating rects are the `Order >= Middle` set the plugin already computes.

**Input.** Each frame owner reports its live rects — its bands, its content rect, its floating
rects. The host routes a pointer event to the topmost frame whose live rects contain it, and lets
it fall through to the layer beneath otherwise. Keyboard goes to the topmost frame owner.

**Previews stay separate.** A block is thumbnailed while it is being edited (the filmstrip proves
it), and previews are drawn on rotated quads. Preview placements keep their own screens, each in
**its own viewport of the same context**.

**Embedded live children** (a video playing inside a canvas, `interaction: Live`) that are not
frame owners keep a screen of their own at their rect, with no chrome. So an instance is exactly
one of: a frame owner, an embedded live screen, or one or more preview screens.

## What this deletes

- Every chrome variant of `EditorRegion`, and per-region screens for the editing set.
- `ChildPart` and `child_part` / `child_part_sized` entirely — nesting is the frame stack now.
- `PluginEditor`'s `direct_editor_top_bar` / `_left_sidebar` / `_right_sidebar` forwarding, and
  the infinite canvas's forwarding of the same (`block_editor.rs:142/199/222`).
- `ChildMode::Passive` / `Active` promotion, if selection into the frame stack subsumes it —
  check `keep_active` and `activate()` callers before removing.
- Presentation's `right_sidebar_ui` pass-through, `self.sidebars`, `self.slide_toolbar`.
- Per-region texture caches in the editors listed under Findings.

## Stages

Commit each stage separately, run `./scripts/verify` before each commit, and push.

**1. One context per instance.** Key `Panes` by `EditorInstanceId`: one context, one
`egui_wgpu::Renderer`, one set of punch resources. Give each region a stable non-ROOT `ViewportId`
and set `input.viewport_id`; never run ROOT. Reset `PunchResources::next` once per instance per
frame and check `punch::SLOTS`. Take the repaint delay from the viewport that ran. Behaviour
unchanged, no protocol change, no host change — a safe base for everything after.

**2. Shared frame layout in `block-ui`.** Lift the layout from `editors.rs:690` into a module that
takes a frame rect and produces bands, including compact mode, resizable sidebars and read-only
scoping, plus the breadcrumb/exit affordance. Convert the host's direct-editor tab to use it,
unchanged visually. Nothing about plugins yet.

**3. Frame protocol.** Replace the chrome regions with a single frame screen. The host sends the
frame rect, which bands this instance owns this frame, the content rect and the
read-only/compact flags; the plugin publishes one screen per frame owner plus its live rects and
floating rects. Bump
`PROTOCOL_VERSION`, document the rule in `crates/block-plugin-api/PROTOCOL.md`, extend `validate`
coverage and the round-trip tests. No compatibility shims — this project does not keep them.

**4. Plugin-side frame pass.** `EguiSession` renders one pass per frame owner: the framework
builds the frame `Ui` from the shared layout and calls `toolbar_ui`, `left_sidebar_ui`,
`right_sidebar_ui` and `ui` into the bands, wrapped in the existing `begin_region`/`end_region`
so children, occluders, drag and file-drop stay attributed. Clip each band's `Ui` to its rect so a
plugin cannot paint outside. Report `RegionSize` per band from each band's `Ui::min_rect`, and
attribute the single `cursor_icon` to the band under the pointer. Preview screens run as separate
viewports in the same context. Repack the surface: one slot per frame owner sized to its frame,
plus slots for previews, packed in 2D — the current vertical stack will exceed the maximum texture
dimension once slots are frame-sized.

**5. Host compositing, input and the stack.** Maintain the frame stack per tab; blit base pieces
and floating pieces as described; route input top-down with fall-through. Keep `host.view()` and
the host's pan/zoom gestures as they are — the host still owns the view, and the framework must
not swallow wheel or pinch over the content rect of a `pan_and_zoom` plugin. Make the infinite
canvas and the text editor frame owners, and delete the canvas's chrome forwarding. The invariant
to test here: selecting or deselecting a child must not change the parent's content rect, its zoom
or its pan by a single pixel.

**6. Presentation and cleanups.** The deck is the outer frame owner (its buttons in its toolbar,
the filmstrip in its left sidebar); the selected slide is the inner frame owner whose content rect
is the stage. It takes over the frame like every other child, so **the filmstrip is hidden while a
slide is being edited** and comes back on Escape. That is an accepted regression for now — the
deck's chrome is gone from the frame, not squeezed. The end state that undoes it is the filmstrip
moving into the deck's content area, alongside the stage, where the slide taking the chrome bands
does not hide it; leave a note saying so. `child_part` disappears; the slide's sidebars are its
own, at the edges of the frame. Delete the per-region texture caches. Rewrite the parts of
`guides/adding_a_plugin_editor.md` that describe per-region contexts, per-region textures and
`child_part` — it is currently the main written record of the old model. Do not edit `README.md`;
mention it in the handoff if it is stale.

## Decisions (defaults if nothing says otherwise)

- **Replacement is the only handover.** There is no second mode and no per-parent choice; do not
  add one. A parent that wants something of its own on screen while a child is being edited has to
  put it in its content area, not in a chrome band.
- **The sub-editor's content stays where the block is** on the canvas rather than being pulled
  into the content band. That keeps canvas editing spatial. Offer a zoom-to-fit affordance instead.
- **`present()`** becomes "frame owner takes the whole frame with no bands", which should simplify
  the presenter path.
- Mark the child's frame visually (a recessed band, the child's block label in its sidebar header)
  so it is always clear which editor a control belongs to. Editing a spreadsheet in a canvas and a
  slide in a deck should look identical.

## Verification

`./scripts/verify` before every commit (it runs clippy --fix and cargo fmt, strips code comments —
write none — and enforces the folder/test layout). Then `./scripts/build --target web`, since this
changes the plugin/guest boundary and the browser runs the same modules in a worker. Drive a
plugin headlessly with
`cargo run --example instantiate -p block-wasm-host -- target/wasm32-wasip1-threads/plugin/<name>.wasm`,
which fails unless the plugin presented a surface. Add tests under the existing per-file
convention for the band layout, the surface packer, the rect-piece splitting used for
compositing, and the protocol round trip. Check `crates/block-ui-test` and `crates/block-e2e`
for tests this invalidates.

Do not run the GUI app and do no manual GUI verification. Push even with GUI verification
outstanding, and say so in the handoff. Commit as `type: message` with a `Co-Authored-By:`
trailer.
