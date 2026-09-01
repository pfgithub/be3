# Task: one egui context per plugin editor instance

Today every region of a plugin editor (main, toolbar, left sidebar, right sidebar, preview)
is drawn by an `egui::Context` of its own. Collapse that to one context per *instance*, so an
instance has one egui state, one font atlas, one texture registry and one paint order across
all of its regions — without giving up the property that the host places the regions, which is
what lets a spreadsheet edited inside an infinite canvas get a full-height sidebar instead of
one squeezed into the block's rect on the canvas.

Everything below the "Findings" heading was verified by reading the code; trust it, but re-check
anything that looks stale before relying on it.

## Findings you don't need to re-derive

- `crates/block-editor-plugin/src/panes.rs` keys `Pane` (context + `egui_wgpu::Renderer` +
  punch resources) by `ScreenId`. That is the thing to change.
- `crates/block-editor-plugin/src/egui_session.rs` — `EguiSession` is already per instance and
  already holds every region's state in `regions: HashMap<EditorRegion, RegionState>`. `run()`
  takes a region and a context, and is called once per screen by the paint loop.
- `crates/block-editor-plugin/src/screens.rs` owns the sessions and calls
  `ScreenLayout::stacked` on every relayout.
- **The plugin owns the surface packing.** `ScreenLayout::stacked`
  (`crates/block-plugin-api/src/lib.rs:72`) runs plugin-side and is sent up as `Message::Layout`
  (`crates/block-editor-plugin/src/runtime.rs`, `replace_surface`). The host samples whatever
  rects it is told. Changing how regions are packed needs no protocol negotiation.
- The host draws each region separately and routes input per region:
  `crates/block-app/src/plugin_host/runtime.rs:375` (`editor_ui`),
  `crates/block-app/src/plugin_host/instances.rs:214` (`report`, screens keyed by
  `(instance, region)`), `crates/block-app/src/editors/plugin.rs` (`region_ui` and the
  `direct_editor_*` impls).
- **Every region of an instance already shares one scale factor** — the host passes its single
  `ui.ctx().pixels_per_point()` for all of them (`plugin_host/runtime.rs:419`).
- **Rotation only happens on the preview path.** The editor path is always axis-aligned
  (`Quad::upright(response.rect)`, `plugin_host/runtime.rs:415`); arbitrary corners occur only in
  `PluginEditor::render` (`editors/plugin.rs`, `context.corners`).
- The infinite canvas already delegates the tab's chrome slots to its focused child editor:
  `crates/block-app/src/editors/infinite_canvas/block_editor.rs:142` (`direct_editor_top_bar`),
  `:199` (left sidebar), `:222` (right sidebar).
- The presentation editor is the hard case. `crates/editors/presentation/src/app.rs` places the
  *same* slide block in four places in one frame: every slide as a `ChildMode::Preview` child in
  the filmstrip (`filmstrip_tile`, :452), the selected slide as a `keep_active` child on the
  stage with the slide's own left sidebar carved 240px out of the stage (`slide_sidebar`, :210),
  the slide's `child_part(Toolbar)` inline in its toolbar row (:639), and its right sidebar as a
  pure pass-through of the slide's `child_part(RightSidebar)` (`right_sidebar_ui`, :658).
- egui is 0.34.3 from crates.io (not vendored). **egui prunes non-ROOT viewports that were not
  "used" when their parent viewport runs** (`egui-0.34.3/src/context.rs:2673`). A viewport whose
  parent is ROOT survives as long as ROOT itself is never run. So: give every viewport a stable
  non-ROOT `ViewportId` and never run `ViewportId::ROOT`.
- `floating_rects` in `egui_session.rs` already collects layers at `Order >= Middle` — that is
  the set of things allowed to escape a region.
- Editors currently duplicate uploads per region because contexts are per region: see
  `crates/editors/image_block/src/app.rs` (`self.panes` keyed by `Pane`, decoding the same image
  once per region) and `crates/editors/pixel_art/src/canvas.rs` (`Pane` with its own
  `TextureHandle`). Also check `map`, `pdf`, `paint_review`, `pixel_ray_tracer`.

## Target design

An instance's regions split into two groups.

**The editing set** — main, toolbar, left sidebar, right sidebar, artifact settings, creation.
These are drawn by **one egui pass into one viewport** whose coordinate space is the tab
(the host's viewport that ultimately contains them). The host tells the plugin where each region
sits inside that space. The plugin allocates **one surface slot** for the instance and draws
every region at its tab-space position inside that slot. Region screens stay separate
`ScreenId`s, but their `ScreenPlacement`s become **sub-rects of the one shared slot**. The host's
per-region blitting and input routing therefore do not change at all — it still asks for
region N and gets region N's pixels.

Area of the slot that no region covers is simply never blitted, so the host's own content (the
canvas, the parent editor, other blocks) shows through. That is the "hole" in the informal
description of this design; it costs nothing to implement because it is just unblitted memory.

**Preview placements** keep their own screens, each in **its own viewport of the same context**.
This is not optional: the presentation editor thumbnails a slide in the filmstrip while the same
instance is being edited on the stage, and previews can be drawn on rotated quads.

**Child surfaces stay below parent surfaces and parents keep punching holes.** Do not invert
z-order. The hole is punched at a point in the parent's paint order, which is what makes
"anything the editor draws after the child covers it" work; that must keep working.

## Invariants that must not break

1. The host places regions. A sub-editor's sidebar is sized by the host, never by the parent's
   available space.
2. `child` / `child_part` / `child_above` semantics as documented in
   `guides/adding_a_plugin_editor.md`, including a parent covering a child by drawing after it.
3. Preview rendering onto arbitrary (rotated) quads.
4. Per-region `RegionSize` reporting, cursors, drag/file-drop targeting, occluders and child
   placements — all still per region.
5. `present()`, `show_region()`, `editable()`, `view()`/pan-and-zoom behaviour.

## Stages

Commit each stage separately, run `./scripts/verify` before each commit, and push.

### Stage 1 — one context per instance, one viewport per region

Behaviour-preserving refactor. No protocol change, no `block-app` change.

- Key `Panes` by `EditorInstanceId` instead of `ScreenId`: one `egui::Context`, one
  `egui_wgpu::Renderer`, one set of punch resources per instance. Reset `PunchResources::next`
  once per instance per frame, not once per screen, and check `punch::SLOTS` is still large
  enough for every region's children combined.
- Give each region a stable non-ROOT `ViewportId` and set `input.viewport_id` in
  `EguiSession::run`. Never run ROOT (see Findings).
- Keep each viewport's `screen_rect` at its current slot-derived rect — nothing else moves.
- Take the repaint delay from the viewport that was run, not `ViewportId::ROOT`
  (`repaint_delay` in `panes.rs`).
- Verify with the headless harness (below) that a plugin still hands over a surface.

Expected win on its own: one font atlas and one texture registry per instance instead of one per
region, plus shared `ctx.data`, caches, animations and drag payload.

### Stage 2 — one viewport for the editing set, in tab space

- **Protocol:** add the region's rect within the instance's tab space, and the tab size, to
  `ScreenRequest`/`ViewportMetrics` in `crates/block-plugin-api`. Bump `PROTOCOL_VERSION`,
  describe the rule in `crates/block-plugin-api/PROTOCOL.md`, extend `validate` coverage and the
  round-trip tests. Do not keep any compatibility shim for older clients.
- **Host:** `editor_ui` (`plugin_host/runtime.rs:375`) already has the region's rect and
  `ui.ctx()`; report the rect relative to the tab viewport's origin and the tab's size along with
  the existing metrics.
- **Plugin:** replace `ScreenLayout::stacked` with a packer that gives each instance one slot —
  the tab rect for an instance with an editing set, the union of its region rects otherwise —
  and places each editing-set screen as a sub-rect of that slot at its tab-space offset.
  Preview screens keep slots of their own. Pack slots in 2D (shelf packing is fine); the current
  vertical stack plus tab-sized slots will exceed the maximum texture dimension on mobile and
  web.
- **Plugin:** `EguiSession::run` becomes one pass per instance that draws every editing-set
  region into its own child `Ui`, clipped to that region's rect, wrapped in the existing
  `begin_region`/`end_region` so children, occluders, drag and file-drop stay region-scoped.
  Region order should be deterministic and documented; main before chrome is the useful order
  because containers compute chrome contents while drawing main.
- Per-region `used` sizes now come from each region's own `Ui::min_rect`.
- There is now one `platform_output.cursor_icon` per instance: attribute it to the region the
  pointer is in and report `Default` for the others.
- Assert that all editing-set screens of an instance share a scale factor; if they ever differ,
  use the largest and log it.
- Input needs no host change: events still arrive per screen in region-local coordinates and
  `EguiSession::input` already offsets them by the region's origin — that origin is now the
  tab-space one.

### Stage 3 — overlay screen for spill

Without this, a menu opened in a toolbar is still trapped in the toolbar strip, and a nested
editor's menu cannot cover its parent (child surfaces are below parent surfaces).

- Add `EditorRegion::Overlay`: a screen whose metrics are the whole tab and whose placement is
  the instance's own slot, so it samples the same pixels the editing pass already drew.
- The plugin reports the floating layers' rects (reuse `floating_rects`) as the overlay's live
  rects, minus the parts that fall inside a region rect — those pixels are already blitted by
  that region. The difference is a handful of axis-aligned pieces; a rect-subtract helper is
  enough. There must be no overlapping blit, or translucent shadows will blend twice.
- The host blits those pieces above the whole instance chain, ordered by focus depth (innermost
  on top), and routes pointer events over them to the overlay screen.
- Allocate the overlay screen lazily — only while the instance has a floating layer.

### Stage 4 — cleanups the single context enables

- Delete per-region texture/decode caches in the editors listed under Findings; one upload per
  instance now suffices.
- Rewrite the parts of `guides/adding_a_plugin_editor.md` that describe per-region contexts and
  per-region textures ("Every region is drawn by an egui context of its own…"), plus anything
  about regions being independent passes. Do not edit `README.md`; say in the handoff if it is
  out of date.

### Stage 5 — presentation and container chrome (UX change)

The single-pass model makes chrome an assignment of rects rather than pixels proxied through the
parent, so the presentation editor can lose machinery:

- Drop `right_sidebar_ui`'s pass-through (:658) — the host can assign the tab's right-sidebar
  rect to the slide instance directly.
- `self.sidebars` and `self.slide_toolbar` become ordinary within-frame data now that region
  order inside a frame is deterministic. (The host still learns `show_region` a frame later;
  that is the process round trip, not the context split. Do not claim otherwise.)

Then make container chrome consistent, because presentation currently uses three different
conventions and the infinite canvas answers the same question separately: **the focused child's
chrome nests inside the parent's, in a recessed band labelled with the child's block name.** The
deck's own toolbar buttons sit outside, the slide's segment inside a subtly inset group; the
right sidebar gets a header naming the child instead of silently becoming the child's sidebar;
the same treatment marks the slide's left sidebar inside the stage. Editing a spreadsheet inside
a canvas and editing a slide inside a deck should then look and behave identically. Put the
shared drawing helpers in `crates/block-ui` so the host and plugins agree.

## Decisions to make (recommendations)

- **May a container hand a child's chrome an arbitrary rect, or only host-offered slots?**
  Recommend arbitrary — `slide_sidebar` already does it and the canvas needs it.
- **Slot size for an instance with an editing set.** Recommend the whole tab, so stage 3 has
  somewhere to put spill. Measure the memory (a 2560×1440 DPR2 tab is ~15 MB) and reconsider on
  mobile if it hurts.
- If stage 3 turns out to be more than a day's work, land stages 1, 2 and 4 first — they stand on
  their own — and report what stage 3 needs.

## Verification

`./scripts/verify` before every commit (it runs clippy --fix and cargo fmt, strips code comments
and enforces the folder/test layout — write no comments). Then `./scripts/build --target web`,
since this changes the plugin/guest boundary and the browser runs the same modules in a worker.
Drive a plugin headlessly with
`cargo run --example instantiate -p block-wasm-host -- target/wasm32-wasip1-threads/plugin/<name>.wasm`,
which fails unless the plugin presented a surface. Add tests under the existing per-file
convention for the packer, the rect-subtract helper and the protocol round trip. Check
`crates/block-ui-test` and `crates/block-e2e` for tests this invalidates.

Do not run the GUI app or do any manual GUI verification; push even if GUI verification is still
outstanding, and say so in the handoff. Commit as `type: message` with a
`Co-Authored-By:` trailer.
